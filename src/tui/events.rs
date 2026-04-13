use chrono::Datelike;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::error::Result;
use crate::models::account::AccountType;

use super::app::{App, ExpenseField, ImportField, Tab};

impl App {
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        // 全局：Ctrl+E 导出，Ctrl+I 打开导入弹窗（优先于其他处理）
        if modifiers == KeyModifiers::CONTROL {
            match code {
                KeyCode::Char('e') => {
                    self.do_export();
                    return Ok(());
                }
                KeyCode::Char('i') => {
                    self.show_import = !self.show_import;
                    if self.show_import {
                        self.import_form.file_path.clear();
                        self.import_form.account_idx = 0;
                        self.import_form.active_field = ImportField::FilePath;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        // 导入弹窗打开时独占键盘
        if self.show_import {
            return self.handle_import_modal(code, modifiers);
        }

        // 全局 Tab 切换（月结编辑状态下不响应）
        if !self.monthly_form.editing && !self.adding_expense && !self.adding_account {
            match code {
                KeyCode::Tab => {
                    self.current_tab = self.current_tab.next();
                    return Ok(());
                }
                KeyCode::BackTab => {
                    self.current_tab = self.current_tab.prev();
                    return Ok(());
                }
                KeyCode::Char('1') => { self.current_tab = Tab::Overview; return Ok(()); }
                KeyCode::Char('2') => {
                    self.refresh_monthly_form();
                    self.current_tab = Tab::MonthlyEntry;
                    return Ok(());
                }
                KeyCode::Char('3') => { self.current_tab = Tab::LargeExpenses; return Ok(()); }
                KeyCode::Char('4') => { self.current_tab = Tab::Accounts; return Ok(()); }
                _ => {}
            }
        }

        match self.current_tab {
            Tab::Overview => self.handle_overview(code)?,
            Tab::MonthlyEntry => self.handle_monthly_entry(code, modifiers)?,
            Tab::LargeExpenses => self.handle_large_expenses(code, modifiers)?,
            Tab::Accounts => self.handle_accounts(code, modifiers)?,
        }
        Ok(())
    }

    fn handle_import_modal(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('s') {
            self.do_import()?;
            return Ok(());
        }
        match code {
            KeyCode::Esc => {
                self.show_import = false;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.import_form.active_field = match self.import_form.active_field {
                    ImportField::FilePath => ImportField::Account,
                    ImportField::Account => ImportField::FilePath,
                };
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.import_form.active_field = match self.import_form.active_field {
                    ImportField::FilePath => ImportField::Account,
                    ImportField::Account => ImportField::FilePath,
                };
            }
            KeyCode::Enter => match self.import_form.active_field {
                ImportField::FilePath => {
                    self.import_form.active_field = ImportField::Account;
                }
                ImportField::Account => {
                    self.do_import()?;
                }
            },
            _ => match self.import_form.active_field {
                ImportField::FilePath => handle_text(&mut self.import_form.file_path, code),
                ImportField::Account => match code {
                    KeyCode::Left => {
                        let len = self.accounts.len();
                        if len > 0 {
                            self.import_form.account_idx =
                                (self.import_form.account_idx + len - 1) % len;
                        }
                    }
                    KeyCode::Right => {
                        let len = self.accounts.len();
                        if len > 0 {
                            self.import_form.account_idx =
                                (self.import_form.account_idx + 1) % len;
                        }
                    }
                    _ => {}
                },
            },
        }
        Ok(())
    }

    fn handle_overview(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down => {
                if self.overview_selected + 1 < self.trend.len() {
                    self.overview_selected += 1;
                }
            }
            KeyCode::Up => {
                self.overview_selected = self.overview_selected.saturating_sub(1);
            }
            KeyCode::Char('r') => {
                self.refresh_trend();
                self.status_msg = Some("已刷新".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_monthly_entry(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // 保存
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('s') {
            if self.monthly_form.entries.is_empty() {
                self.status_msg = Some("❌ 无账户可记录".into());
                return Ok(());
            }
            self.save_monthly_snapshot()?;
            return Ok(());
        }

        if self.monthly_form.editing {
            match code {
                KeyCode::Enter | KeyCode::Tab => {
                    self.monthly_form.confirm_current();
                }
                KeyCode::Esc => {
                    self.monthly_form.editing = false;
                    self.monthly_form.input_buf.clear();
                }
                KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                    self.monthly_form.input_buf.push(c);
                }
                KeyCode::Backspace => {
                    self.monthly_form.input_buf.pop();
                }
                _ => {}
            }
        } else {
            match code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Down => {
                    if self.monthly_form.selected + 1 < self.monthly_form.entries.len() {
                        self.monthly_form.selected += 1;
                    }
                }
                KeyCode::Up => {
                    self.monthly_form.selected = self.monthly_form.selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    // 开始编辑当前账户余额
                    if let Some(item) = self.monthly_form.entries.get(self.monthly_form.selected) {
                        // 预填上次余额
                        self.monthly_form.input_buf = item
                            .confirmed_balance
                            .or(item.last_balance)
                            .map(|b| format!("{b:.2}"))
                            .unwrap_or_default();
                    }
                    self.monthly_form.editing = true;
                }
                // 切换月份
                KeyCode::Left => {
                    let (y, m) = prev_month(self.monthly_form.year, self.monthly_form.month);
                    self.monthly_form.year = y;
                    self.monthly_form.month = m;
                    self.refresh_monthly_form();
                }
                KeyCode::Right => {
                    let now = chrono::Local::now();
                    let (y, m) = next_month(self.monthly_form.year, self.monthly_form.month);
                    // 不允许跳到未来月份
                    if y < now.year() || (y == now.year() && m <= now.month()) {
                        self.monthly_form.year = y;
                        self.monthly_form.month = m;
                        self.refresh_monthly_form();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_large_expenses(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.adding_expense {
            return self.handle_expense_form(code, modifiers);
        }
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down => {
                if self.expense_selected + 1 < self.large_expenses.len() {
                    self.expense_selected += 1;
                }
            }
            KeyCode::Up => {
                self.expense_selected = self.expense_selected.saturating_sub(1);
            }
            KeyCode::Char('a') => {
                self.expense_form.reset();
                self.adding_expense = true;
            }
            KeyCode::Char('d') => {
                self.delete_selected_expense()?;
            }
            KeyCode::Char('r') => {
                self.refresh_large_expenses();
                self.status_msg = Some("已刷新".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_expense_form(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('s') {
            self.submit_expense_form()?;
            return Ok(());
        }
        match code {
            KeyCode::Esc => {
                self.adding_expense = false;
                self.expense_form.reset();
            }
            KeyCode::Tab => {
                self.expense_form.active_field = self.expense_form.active_field.next();
            }
            KeyCode::BackTab => {
                self.expense_form.active_field = self.expense_form.active_field.prev();
            }
            KeyCode::Enter => {
                if self.expense_form.active_field == ExpenseField::Note {
                    self.submit_expense_form()?;
                } else {
                    self.expense_form.active_field = self.expense_form.active_field.next();
                }
            }
            _ => match self.expense_form.active_field {
                ExpenseField::Amount => handle_text(&mut self.expense_form.amount, code),
                ExpenseField::Category => handle_list_nav(
                    &mut self.expense_form.category_idx,
                    self.expense_form.categories.len(),
                    code,
                ),
                ExpenseField::Account => handle_list_nav(
                    &mut self.expense_form.account_idx,
                    self.expense_form.accounts.len(),
                    code,
                ),
                ExpenseField::Date => handle_text(&mut self.expense_form.date, code),
                ExpenseField::Note => handle_text(&mut self.expense_form.note, code),
            },
        }
        Ok(())
    }

    fn handle_accounts(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.adding_account {
            return self.handle_new_account_form(code);
        }
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down => {
                if self.acc_selected + 1 < self.accounts.len() {
                    self.acc_selected += 1;
                }
            }
            KeyCode::Up => {
                self.acc_selected = self.acc_selected.saturating_sub(1);
            }
            KeyCode::Char('a') => {
                self.adding_account = true;
                self.new_acc_name.clear();
                self.new_acc_type_idx = 0;
            }
            KeyCode::Char('d') => {
                if let Some(acc) = self.accounts.get(self.acc_selected) {
                    let id = acc.id;
                    match self.service.delete_account(id) {
                        Ok(()) => {
                            self.refresh_accounts();
                            self.refresh_monthly_form();
                            self.acc_selected = self.acc_selected.saturating_sub(1);
                            self.status_msg = Some("🗑 账户已删除".into());
                        }
                        Err(_) => {
                            self.status_msg = Some("❌ 无法删除（存在关联账单）".into());
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_new_account_form(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => {
                self.adding_account = false;
                self.new_acc_name.clear();
            }
            KeyCode::Enter => {
                self.submit_new_account()?;
            }
            KeyCode::Left | KeyCode::Up => {
                let len = AccountType::all().len();
                self.new_acc_type_idx = (self.new_acc_type_idx + len - 1) % len;
            }
            KeyCode::Right | KeyCode::Down => {
                let len = AccountType::all().len();
                self.new_acc_type_idx = (self.new_acc_type_idx + 1) % len;
            }
            _ => handle_text(&mut self.new_acc_name, code),
        }
        Ok(())
    }
}

fn handle_text(field: &mut String, code: KeyCode) {
    match code {
        KeyCode::Char(c) => field.push(c),
        KeyCode::Backspace => { field.pop(); }
        _ => {}
    }
}

fn handle_list_nav(idx: &mut usize, len: usize, code: KeyCode) {
    if len == 0 { return; }
    match code {
        KeyCode::Down => *idx = (*idx + 1) % len,
        KeyCode::Up => *idx = (*idx + len - 1) % len,
        _ => {}
    }
}

fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 { (year - 1, 12) } else { (year, month - 1) }
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 { (year + 1, 1) } else { (year, month + 1) }
}
