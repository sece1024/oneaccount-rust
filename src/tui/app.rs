use std::sync::Arc;
use std::time::Duration;

use chrono::Datelike;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::Result;
use crate::models::account::{Account, AccountType, NewAccount};
use crate::models::category::Category;
use crate::models::snapshot::{MonthlyEntryItem, MonthlyTotal};
use crate::models::transaction::{NewTransaction, Transaction, TransactionType};
use crate::service::AppService;

// ── 草稿相关 ──────────────────────────────────────────────────────────────────

/// 月结草稿
#[derive(Serialize, Deserialize)]
pub struct MonthlyDraft {
    pub year: i32,
    pub month: u32,
    pub entries: Vec<DraftEntry>,
    pub saved_at: String,
}

/// 草稿条目
#[derive(Serialize, Deserialize)]
pub struct DraftEntry {
    pub account_id: i64,
    pub balance: f64,
}

// ── CSV 导入表单 ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportField {
    FilePath,
    Account,
}

pub struct ImportForm {
    pub file_path: String,
    pub account_idx: usize,
    pub active_field: ImportField,
}

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Overview = 0,      // 资产总览
    MonthlyEntry = 1,  // 月结
    LargeExpenses = 2, // 大额支出
    Accounts = 3,      // 账户管理
}

impl Tab {
    pub fn next(self) -> Tab {
        match self {
            Tab::Overview => Tab::MonthlyEntry,
            Tab::MonthlyEntry => Tab::LargeExpenses,
            Tab::LargeExpenses => Tab::Accounts,
            Tab::Accounts => Tab::Overview,
        }
    }
    pub fn prev(self) -> Tab {
        match self {
            Tab::Overview => Tab::Accounts,
            Tab::MonthlyEntry => Tab::Overview,
            Tab::LargeExpenses => Tab::MonthlyEntry,
            Tab::Accounts => Tab::LargeExpenses,
        }
    }
    pub fn names() -> [&'static str; 4] {
        ["📊 资产总览", "📅 月结", "💸 大额支出", "💳 账户管理"]
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

// ── 月结表单 ──────────────────────────────────────────────────────────────────

/// 撤销操作类型
#[derive(Clone)]
pub enum UndoAction {
    /// 编辑前的状态
    Edit {
        account_id: i64,
        old_balance: Option<f64>,
        new_balance: Option<f64>,
    },
    /// 确认前的状态
    Confirm {
        account_id: i64,
        old_confirmed: Option<f64>,
    },
}

pub struct MonthlyForm {
    pub year: i32,
    pub month: u32,
    pub entries: Vec<MonthlyEntryItem>,
    pub selected: usize,
    pub editing: bool,      // 当前是否在编辑某一行的余额
    pub input_buf: String,  // 输入缓冲区
    pub saved: bool,        // 本月是否已保存过
    pub undo_stack: Vec<UndoAction>, // 撤销栈
}

impl MonthlyForm {
    pub fn new(year: i32, month: u32, entries: Vec<MonthlyEntryItem>, saved: bool) -> Self {
        Self {
            year,
            month,
            entries,
            selected: 0,
            editing: false,
            input_buf: String::new(),
            saved,
            undo_stack: Vec::new(),
        }
    }

    /// 记录编辑操作（用于撤销）
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        // 限制栈大小，避免内存占用过多
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// 撤销上一步操作
    pub fn undo(&mut self) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            match action {
                UndoAction::Edit {
                    account_id,
                    old_balance,
                    ..
                } => {
                    if let Some(item) = self.entries.iter_mut().find(|e| e.account_id == account_id)
                    {
                        item.confirmed_balance = old_balance;
                        item.input = old_balance
                            .map(|b| format!("{b:.2}"))
                            .unwrap_or_default();
                    }
                }
                UndoAction::Confirm {
                    account_id,
                    old_confirmed,
                } => {
                    if let Some(item) = self.entries.iter_mut().find(|e| e.account_id == account_id)
                    {
                        item.confirmed_balance = old_confirmed;
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub fn confirm_current(&mut self) {
        // 记录撤销操作
        if let Some(item) = self.entries.get(self.selected) {
            self.push_undo(UndoAction::Confirm {
                account_id: item.account_id,
                old_confirmed: item.confirmed_balance,
            });
        }

        if let Ok(val) = self.input_buf.trim().parse::<f64>() {
            if let Some(item) = self.entries.get_mut(self.selected) {
                item.confirmed_balance = Some(val);
                item.input = self.input_buf.clone();
            }
        }
        self.editing = false;
        self.input_buf.clear();
        // 自动移到下一项
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn confirmed_total(&self) -> f64 {
        self.entries
            .iter()
            .filter_map(|e| e.confirmed_balance)
            .sum()
    }

    #[allow(dead_code)]
    pub fn all_confirmed(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|e| e.confirmed_balance.is_some())
    }

    pub fn confirmed_count(&self) -> usize {
        self.entries.iter().filter(|e| e.confirmed_balance.is_some()).count()
    }
}

// ── 大额支出表单 ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpenseField {
    Amount,
    Category,
    Account,
    Date,
    Note,
}

impl ExpenseField {
    pub fn next(self) -> Self {
        match self {
            ExpenseField::Amount => ExpenseField::Category,
            ExpenseField::Category => ExpenseField::Account,
            ExpenseField::Account => ExpenseField::Date,
            ExpenseField::Date => ExpenseField::Note,
            ExpenseField::Note => ExpenseField::Amount,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            ExpenseField::Amount => ExpenseField::Note,
            ExpenseField::Category => ExpenseField::Amount,
            ExpenseField::Account => ExpenseField::Category,
            ExpenseField::Date => ExpenseField::Account,
            ExpenseField::Note => ExpenseField::Date,
        }
    }
}

pub struct ExpenseForm {
    pub amount: String,
    pub category_idx: usize,
    pub account_idx: usize,
    pub date: String,
    pub note: String,
    pub active_field: ExpenseField,
    pub categories: Vec<Category>,
    pub accounts: Vec<Account>,
}

impl ExpenseForm {
    pub fn new(categories: Vec<Category>, accounts: Vec<Account>) -> Self {
        Self {
            amount: String::new(),
            category_idx: 0,
            account_idx: 0,
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            note: String::new(),
            active_field: ExpenseField::Amount,
            categories,
            accounts,
        }
    }

    pub fn validate(&self) -> Option<String> {
        if self.amount.trim().parse::<f64>().is_err() {
            return Some("金额必须是有效数字".into());
        }
        if self.accounts.is_empty() {
            return Some("请先创建账户".into());
        }
        if chrono::NaiveDate::parse_from_str(self.date.trim(), "%Y-%m-%d").is_err() {
            return Some("日期格式错误（YYYY-MM-DD）".into());
        }
        None
    }

    pub fn to_new_transaction(&self) -> Option<NewTransaction> {
        let amount = self.amount.trim().parse::<f64>().ok()?;
        let account_id = self.accounts.get(self.account_idx)?.id;
        let category_id = self.categories.get(self.category_idx).map(|c| c.id);
        Some(NewTransaction {
            amount,
            transaction_type: TransactionType::Expense,
            category_id,
            account_id,
            to_account_id: None,
            date: self.date.trim().to_string(),
            note: if self.note.is_empty() { None } else { Some(self.note.clone()) },
            is_large: true,
        })
    }

    pub fn reset(&mut self) {
        self.amount.clear();
        self.note.clear();
        self.category_idx = 0;
        self.account_idx = 0;
        self.date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.active_field = ExpenseField::Amount;
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub service: AppService,
    pub current_tab: Tab,
    pub should_quit: bool,
    pub status_msg: Option<String>,

    // 资产总览
    pub trend: Vec<MonthlyTotal>,
    pub overview_selected: usize,

    // 月结
    pub monthly_form: MonthlyForm,

    // 大额支出
    pub large_expenses: Vec<Transaction>,
    pub expense_selected: usize,
    pub expense_form: ExpenseForm,
    pub adding_expense: bool,

    // 账户管理
    pub accounts: Vec<Account>,
    pub acc_selected: usize,
    pub adding_account: bool,
    pub new_acc_name: String,
    pub new_acc_type_idx: usize,

    // CSV 导入弹窗
    pub show_import: bool,
    pub import_form: ImportForm,
}

impl App {
    pub fn new(pool: Arc<DbPool>) -> Result<Self> {
        let service = AppService::new(pool);
        let now = chrono::Local::now();

        let trend = service.asset_trend(24).unwrap_or_default();
        let entries = service.build_monthly_entry_items()?;
        let saved = service.has_snapshot(now.year(), now.month())?;
        let monthly_form = MonthlyForm::new(now.year(), now.month(), entries, saved);

        let large_expenses = service.list_large_expenses().unwrap_or_default();
        let accounts = service.list_accounts()?;
        let categories = service.list_categories().unwrap_or_default();
        // 只保留支出分类供大额支出表单使用
        let expense_cats: Vec<_> = categories
            .into_iter()
            .filter(|c| c.category_type == crate::models::category::CategoryType::Expense)
            .collect();
        let expense_form = ExpenseForm::new(expense_cats, accounts.clone());

        Ok(Self {
            service,
            current_tab: Tab::Overview,
            should_quit: false,
            status_msg: None,
            trend,
            overview_selected: 0,
            monthly_form,
            large_expenses,
            expense_selected: 0,
            expense_form,
            adding_expense: false,
            accounts,
            acc_selected: 0,
            adding_account: false,
            new_acc_name: String::new(),
            new_acc_type_idx: 0,
            show_import: false,
            import_form: ImportForm {
                file_path: String::new(),
                account_idx: 0,
                active_field: ImportField::FilePath,
            },
        })
    }

    pub fn refresh_trend(&mut self) {
        self.trend = self.service.asset_trend(24).unwrap_or_default();
    }

    pub fn refresh_monthly_form(&mut self) {
        let year = self.monthly_form.year;
        let month = self.monthly_form.month;
        if let Ok(entries) = self.service.build_monthly_entry_items() {
            let saved = self.service.has_snapshot(year, month).unwrap_or(false);
            self.monthly_form = MonthlyForm::new(year, month, entries, saved);
        }
        // 预填：若已有本月快照，从快照加载余额
        if let Ok(snaps) = self.service.month_snapshots(year, month) {
            for snap in &snaps {
                if let Some(item) = self
                    .monthly_form
                    .entries
                    .iter_mut()
                    .find(|e| e.account_id == snap.account_id)
                {
                    item.confirmed_balance = Some(snap.balance);
                    item.input = format!("{:.2}", snap.balance);
                }
            }
        }
        // 尝试加载草稿（仅未保存时）
        if !self.monthly_form.saved {
            let _ = self.load_draft();
        }
    }

    pub fn refresh_large_expenses(&mut self) {
        self.large_expenses = self.service.list_large_expenses().unwrap_or_default();
    }

    pub fn refresh_accounts(&mut self) {
        if let Ok(accs) = self.service.list_accounts() {
            self.accounts = accs.clone();
            self.expense_form.accounts = accs;
        }
    }

    pub fn save_monthly_snapshot(&mut self) -> Result<()> {
        let year = self.monthly_form.year;
        let month = self.monthly_form.month;
        let total = self
            .service
            .save_monthly_snapshot(year, month, &self.monthly_form.entries, None)?;
        self.monthly_form.saved = true;
        self.refresh_trend();
        self.refresh_accounts();
        self.status_msg = Some(format!(
            "✅ {year}-{month:02} 月结保存成功，总资产 ¥{total:.2}"
        ));
        Ok(())
    }

    /// 保存草稿到本地文件
    pub fn save_draft(&mut self) -> Result<()> {
        let draft = MonthlyDraft {
            year: self.monthly_form.year,
            month: self.monthly_form.month,
            entries: self
                .monthly_form
                .entries
                .iter()
                .filter(|e| e.confirmed_balance.is_some())
                .map(|e| DraftEntry {
                    account_id: e.account_id,
                    balance: e.confirmed_balance.unwrap(),
                })
                .collect(),
            saved_at: chrono::Local::now().to_rfc3339(),
        };

        let draft_path = self.draft_path();
        // 确保目录存在
        if let Some(parent) = draft_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(&draft_path)?;
        serde_json::to_writer_pretty(file, &draft)?;
        self.status_msg = Some("💾 草稿已保存".into());
        Ok(())
    }

    /// 加载草稿
    pub fn load_draft(&mut self) -> Result<()> {
        let draft_path = self.draft_path();
        if !draft_path.exists() {
            return Ok(());
        }

        let file = std::fs::File::open(&draft_path)?;
        let draft: MonthlyDraft = serde_json::from_reader(file)?;

        // 验证是否是当前月份的草稿
        if draft.year == self.monthly_form.year && draft.month == self.monthly_form.month {
            for entry in &draft.entries {
                if let Some(item) = self
                    .monthly_form
                    .entries
                    .iter_mut()
                    .find(|e| e.account_id == entry.account_id)
                {
                    item.confirmed_balance = Some(entry.balance);
                    item.input = format!("{:.2}", entry.balance);
                }
            }
            self.status_msg = Some("📂 已恢复草稿".into());
        }

        Ok(())
    }

    /// 获取草稿文件路径
    fn draft_path(&self) -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("oneaccount")
            .join("drafts")
            .join(format!(
                "draft_{}_{}.json",
                self.monthly_form.year, self.monthly_form.month
            ))
    }

    pub fn submit_expense_form(&mut self) -> Result<()> {
        if let Some(err) = self.expense_form.validate() {
            self.status_msg = Some(format!("❌ {err}"));
            return Ok(());
        }
        if let Some(req) = self.expense_form.to_new_transaction() {
            self.service.create_transaction(&req)?;
            self.expense_form.reset();
            self.adding_expense = false;
            self.refresh_large_expenses();
            self.refresh_accounts();
            self.status_msg = Some("✅ 大额支出已记录".into());
        }
        Ok(())
    }

    pub fn delete_selected_expense(&mut self) -> Result<()> {
        if let Some(tx) = self.large_expenses.get(self.expense_selected) {
            let id = tx.id;
            self.service.delete_transaction(id)?;
            self.refresh_large_expenses();
            self.refresh_accounts();
            self.expense_selected = self.expense_selected.saturating_sub(1);
            self.status_msg = Some("🗑 已删除".into());
        }
        Ok(())
    }

    pub fn do_export(&mut self) {
        let now = chrono::Local::now();
        let filename = format!("oneaccount_export_{}.csv", now.format("%Y%m%d_%H%M%S"));
        match self.service.export_csv(&filename) {
            Ok(n) => self.status_msg = Some(format!("✅ 已导出 {n} 条账单 → {filename}")),
            Err(e) => self.status_msg = Some(format!("❌ 导出失败: {e}")),
        }
    }

    pub fn do_import(&mut self) -> Result<()> {
        let path = self.import_form.file_path.trim().to_string();
        if path.is_empty() {
            self.status_msg = Some("❌ 请输入文件路径".into());
            return Ok(());
        }
        let account_id = match self.accounts.get(self.import_form.account_idx) {
            Some(a) => a.id,
            None => {
                self.status_msg = Some("❌ 请先选择导入账户".into());
                return Ok(());
            }
        };
        match self.service.import_csv(&path, account_id) {
            Ok(n) => {
                self.show_import = false;
                self.import_form.file_path.clear();
                self.refresh_large_expenses();
                self.refresh_accounts();
                self.status_msg = Some(format!("✅ 成功导入 {n} 条账单"));
            }
            Err(e) => {
                self.status_msg = Some(format!("❌ 导入失败: {e}"));
            }
        }
        Ok(())
    }

    pub fn submit_new_account(&mut self) -> Result<()> {        let name = self.new_acc_name.trim().to_string();
        if name.is_empty() {
            self.status_msg = Some("❌ 账户名称不能为空".into());
            return Ok(());
        }
        let all_types = AccountType::all();
        let acc_type = all_types[self.new_acc_type_idx % all_types.len()].clone();
        self.service.create_account(&NewAccount {
            name,
            account_type: acc_type,
            currency: "CNY".into(),
            initial_balance: 0.0,
            is_liquid: true,
        })?;
        self.new_acc_name.clear();
        self.adding_account = false;
        self.refresh_accounts();
        // 同步月结表单账户列表
        self.refresh_monthly_form();
        self.status_msg = Some("✅ 账户已创建".into());
        Ok(())
    }

    // ── 主事件循环 ────────────────────────────────────────────────────────────

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| super::ui::render(f, self))?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key.code, key.modifiers)?;
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }
}
