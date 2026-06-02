use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
};

use super::app::{App, ExpenseField, ImportField, Tab};

// ── 颜色主题 ───────────────────────────────────────────────────────────────────
const CYAN: Color = Color::Cyan;
const GREEN: Color = Color::Green;
const RED: Color = Color::Red;
const YELLOW: Color = Color::Yellow;
const DARK: Color = Color::DarkGray;

fn title_style() -> Style { Style::default().fg(CYAN).add_modifier(Modifier::BOLD) }

/// 将余额格式化为带货币符号的字符串，负数显示为 -¥1234.56
fn fmt_balance(b: f64) -> String {
    if b < 0.0 {
        format!("-¥{:.2}", b.abs())
    } else {
        format!("¥{:.2}", b)
    }
}

fn fmt_percent(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn fmt_growth(v: Option<f64>) -> (String, Color) {
    match v {
        Some(value) if value > 0.0 => (format!("↑{:.1}%", value * 100.0), GREEN),
        Some(value) if value < 0.0 => (format!("↓{:.1}%", value.abs() * 100.0), RED),
        Some(_) => ("→0.0%".into(), Color::White),
        None => ("—".into(), DARK),
    }
}

fn active_style() -> Style { Style::default().fg(YELLOW).add_modifier(Modifier::BOLD) }
fn sel_style() -> Style { Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD) }

// ── 顶层渲染入口 ───────────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    render_tabs(frame, app, chunks[0]);
    render_help(frame, app, chunks[2]);

    match app.current_tab {
        Tab::Overview => render_overview(frame, app, chunks[1]),
        Tab::MonthlyEntry => render_monthly_entry(frame, app, chunks[1]),
        Tab::LargeExpenses => render_large_expenses(frame, app, chunks[1]),
        Tab::Accounts => render_accounts(frame, app, chunks[1]),
        Tab::Analytics => render_analytics(frame, app, chunks[1]),
    }

    // 导入弹窗覆盖在最上层
    if app.show_import {
        render_import_modal(frame, app, area);
    }

    // 清空确认弹窗覆盖在最上层
    if app.show_clear_confirm {
        render_clear_confirm_modal(frame, area);
    }
}

// ── Tab 栏 ────────────────────────────────────────────────────────────────────

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs = Tabs::new(Tab::names().map(Line::from))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" OneAccount 💰 ")
                .title_style(title_style()),
        )
        .select(app.current_tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(active_style().add_modifier(Modifier::UNDERLINED));
    frame.render_widget(tabs, area);
}

// ── 帮助栏 ────────────────────────────────────────────────────────────────────

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let hint = if app.show_clear_confirm {
        "⚠️  清空所有数据  │  [y]确认清空  [Esc/n]取消"
    } else if app.show_import {
        "[Tab]切换字段  [←→]选择账户  [Enter/Ctrl+S]确认导入  [Esc]取消"
    } else {
        match app.current_tab {
            Tab::Overview =>
                "[q]退出 [Tab]切换 [↑↓]选择 [r]刷新 [Ctrl+E]导出CSV [Ctrl+I]导入CSV [Ctrl+X]清空数据",
            Tab::MonthlyEntry =>
                "[Enter]编辑余额 [Tab/Enter]下一个 [←→]切换月份 [Ctrl+S]保存月结 [Ctrl+E]导出 [Ctrl+I]导入",
            Tab::LargeExpenses =>
                "[a]新增 [d]删除 [↑↓]导航 [Ctrl+E]导出CSV [Ctrl+I]导入CSV [Ctrl+X]清空数据",
            Tab::Accounts =>
                "[a]新建账户 [d]删除 [↑↓]导航 [Ctrl+E]导出CSV [Ctrl+I]导入CSV [Ctrl+X]清空数据",
            Tab::Analytics =>
                "[←→]切换月份 [Tab]切换标签 [q]退出 [Ctrl+E]导出CSV [Ctrl+I]导入CSV [Ctrl+X]清空数据",
        }
    };
    let msg = if let Some(s) = &app.status_msg {
        format!("{hint}  │  {s}")
    } else {
        hint.to_string()
    };
    frame.render_widget(
        Paragraph::new(msg).style(Style::default().fg(DARK)),
        area,
    );
}

// ── 资产总览 ───────────────────────────────────────────────────────────────────

fn render_overview(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(12)]).split(area);

    // 上方：12 个月趋势表
    render_trend_table(frame, app, chunks[0]);
    // 下方：最新月快照各账户明细
    render_latest_snapshot(frame, app, chunks[1]);
}

fn render_trend_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 📈 总资产趋势（最近24个月）")
        .title_style(title_style());

    if app.trend.is_empty() {
        frame.render_widget(
            Paragraph::new("暂无数据，请先完成月结（Tab 2）")
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(DARK)),
            area,
        );
        return;
    }

    let rows: Vec<Row> = app
        .trend
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let prev_total = if i == 0 { None } else { app.trend.get(i - 1).map(|p| p.total) };
            let (delta_str, delta_color) = match prev_total {
                None => ("—".into(), Color::White),
                Some(prev) => {
                    let diff = t.total - prev;
                    if diff >= 0.0 {
                        (format!("+¥{diff:.2}"), GREEN)
                    } else {
                        (format!("-¥{:.2}", diff.abs()), RED)
                    }
                }
            };
            let style = if i == app.overview_selected { sel_style() } else { Style::default() };
            Row::new([
                Cell::from(format!("{}-{:02}", t.year, t.month)),
                Cell::from(Line::styled(
                    fmt_balance(t.total),
                    Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                ).alignment(Alignment::Right)),
                Cell::from(Line::styled(delta_str, Style::default().fg(delta_color))
                    .alignment(Alignment::Right)),
            ])
            .style(style)
        })
        .collect();

    let header = Row::new(["月份", "总资产", "环比变化"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let widths = [Constraint::Length(10), Constraint::Length(18), Constraint::Min(0)];
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(sel_style());

    let mut state = ratatui::widgets::TableState::default().with_selected(Some(app.overview_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_latest_snapshot(frame: &mut Frame, app: &App, area: Rect) {
    // 显示最新一个月的快照明细
    let latest = app.trend.last();
    let title = match latest {
        Some(t) => format!(" 💳 最新快照明细（{}-{:02}）", t.year, t.month),
        None => " 💳 最新账户余额".into(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(title_style());

    // 直接用当前账户余额展示（即最后一次月结同步的值）
    if app.accounts.is_empty() {
        frame.render_widget(
            Paragraph::new("暂无账户").block(block).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|acc| {
            let color = if acc.balance >= 0.0 { GREEN } else { RED };
            Row::new([
                Cell::from(acc.name.clone()),
                Cell::from(acc.account_type.display_name()),
                Cell::from(Line::styled(
                    fmt_balance(acc.balance),
                    Style::default().fg(color),
                ).alignment(Alignment::Right)),
            ])
        })
        .collect();

    let total: f64 = app.accounts.iter().map(|a| a.balance).sum();
    let total_color = if total >= 0.0 { GREEN } else { RED };
    let total_row = Row::new([
        Cell::from(Span::styled("合计", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(""),
        Cell::from(Line::styled(
            fmt_balance(total),
            Style::default().fg(total_color).add_modifier(Modifier::BOLD),
        ).alignment(Alignment::Right)),
    ]);

    let header = Row::new(["账户", "类型", "余额"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let widths = [Constraint::Percentage(40), Constraint::Percentage(20), Constraint::Percentage(40)];

    let mut all_rows = rows;
    all_rows.push(Row::new(["──────────", "", "──────────"]));
    all_rows.push(total_row);

    let table = Table::new(all_rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

// ── 月结 ──────────────────────────────────────────────────────────────────────

fn render_monthly_entry(frame: &mut Frame, app: &App, area: Rect) {
    let form = &app.monthly_form;
    let saved_tag = if form.saved { " ✅已保存" } else { "" };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " 📅 月结  ◀ {}-{:02} ▶{}  [←→切换月份]",
            form.year, form.month, saved_tag
        ))
        .title_style(title_style());

    if form.entries.is_empty() {
        frame.render_widget(
            Paragraph::new("暂无账户，请先到「账户管理」标签创建账户")
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(DARK)),
            area,
        );
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 分割：账户列表 + 底部合计
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(inner);

    // 账户列表
    let rows: Vec<Row> = form
        .entries
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_sel = i == form.selected;
            let is_editing = is_sel && form.editing;

            let new_val = if is_editing {
                // 正在输入中，显示输入框
                if form.input_buf.starts_with('-') {
                    format!("-¥{}█", &form.input_buf[1..])
                } else {
                    format!("¥{}█", form.input_buf)
                }
            } else {
                item.display_new()
            };

            let delta = item.delta_str().unwrap_or_default();
            let delta_color = if delta.starts_with('+') { GREEN } else if delta.starts_with('-') { RED } else { Color::White };

            let row_style = if is_sel { sel_style() } else { Style::default() };
            let new_style = if is_editing {
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
            } else if item.confirmed_balance.is_some() {
                Style::default().fg(GREEN)
            } else {
                Style::default().fg(DARK)
            };

            Row::new([
                Cell::from(if is_sel { format!("▶ {}", item.account_name) } else { format!("  {}", item.account_name) }),
                Cell::from(item.account_type.clone()),
                Cell::from(Line::from(item.display_last()).alignment(Alignment::Right)),
                Cell::from(Line::styled(new_val, new_style).alignment(Alignment::Right)),
                Cell::from(Line::styled(delta, Style::default().fg(delta_color)).alignment(Alignment::Right)),
            ])
            .style(row_style)
        })
        .collect();

    let header = Row::new(["账户", "类型", "上次余额", "本次余额", "变化"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let widths = [
        Constraint::Percentage(22),
        Constraint::Percentage(12),
        Constraint::Percentage(18),
        Constraint::Percentage(22),
        Constraint::Percentage(26),
    ];
    let table = Table::new(rows, widths).header(header).row_highlight_style(sel_style());
    frame.render_widget(table, chunks[0]);

    // 底部合计
    let confirmed = form.confirmed_count();
    let total = form.confirmed_total();
    let hint = if form.editing {
        "[Enter]确认  [Esc]取消输入  [数字/.]输入余额  [-]负数（负债）".into()
    } else {
        format!(
            "已填 {}/{} 个账户  合计 {}  [Enter]编辑余额  [Ctrl+S]保存",
            confirmed,
            form.entries.len(),
            fmt_balance(total)
        )
    };
    frame.render_widget(
        Paragraph::new(hint)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().fg(if form.editing { YELLOW } else { CYAN })),
        chunks[1],
    );
}

fn render_analytics(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📈 财务分析 {}-{:02} ", app.analytics_year, app.analytics_month))
        .title_style(title_style());

    let (comparison, health, structure) = match (
        &app.analytics_comparison,
        &app.analytics_health,
        &app.analytics_structure,
    ) {
        (Some(comparison), Some(health), Some(structure)) => (comparison, health, structure),
        _ => {
            frame.render_widget(
                Paragraph::new("暂无分析数据，请先完成月结")
                    .block(block)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(DARK)),
                area,
            );
            return;
        }
    };

    let (assets_mom, assets_mom_color) = fmt_growth(comparison.assets_mom);
    let (assets_yoy, assets_yoy_color) = fmt_growth(comparison.assets_yoy);
    let liabilities_mom = calc_growth_display(
        comparison.current.total_liabilities,
        comparison.prev_month.total_liabilities,
    );
    let liabilities_yoy = calc_growth_display(
        comparison.current.total_liabilities,
        comparison.last_year.total_liabilities,
    );
    let (net_mom, net_mom_color) = fmt_growth(comparison.net_worth_mom);
    let (net_yoy, net_yoy_color) = fmt_growth(comparison.net_worth_yoy);

    let liquid_ratio = structure
        .by_liquid
        .iter()
        .find(|item| item.category == "流动资产")
        .map(|item| format!("{} ({})", fmt_balance(item.amount), fmt_percent(item.percentage)))
        .unwrap_or_else(|| "¥0.00 (0.0%)".into());
    let illiquid_ratio = structure
        .by_liquid
        .iter()
        .find(|item| item.category == "非流动资产")
        .map(|item| format!("{} ({})", fmt_balance(item.amount), fmt_percent(item.percentage)))
        .unwrap_or_else(|| "¥0.00 (0.0%)".into());

    let health_color = if health.health_score >= 80.0 {
        GREEN
    } else if health.health_score >= 60.0 {
        CYAN
    } else if health.health_score >= 40.0 {
        YELLOW
    } else {
        RED
    };
    let net_color = if comparison.current.net_worth >= 0.0 { GREEN } else { RED };

    let lines = vec![
        Line::from(vec![Span::styled(
            format!("总资产: {}", fmt_balance(comparison.current.total_assets)),
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        ), Span::raw("  "), Span::styled(assets_mom, Style::default().fg(assets_mom_color)), Span::raw(" 环比  "), Span::styled(assets_yoy, Style::default().fg(assets_yoy_color)), Span::raw(" 同比")]),
        Line::from(vec![Span::styled(
            format!("总负债: {}", fmt_balance(comparison.current.total_liabilities)),
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ), Span::raw("  "), Span::styled(liabilities_mom.0, Style::default().fg(liabilities_mom.1)), Span::raw(" 环比  "), Span::styled(liabilities_yoy.0, Style::default().fg(liabilities_yoy.1)), Span::raw(" 同比")]),
        Line::from(vec![Span::styled(
            format!("净资产: {}", fmt_balance(comparison.current.net_worth)),
            Style::default().fg(net_color).add_modifier(Modifier::BOLD),
        ), Span::raw("  "), Span::styled(net_mom, Style::default().fg(net_mom_color)), Span::raw(" 环比  "), Span::styled(net_yoy, Style::default().fg(net_yoy_color)), Span::raw(" 同比")]),
        Line::from(""),
        Line::from(Span::styled("资产结构:", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))),
        Line::from(format!("  流动资产: {liquid_ratio}")),
        Line::from(format!("  非流动资产: {illiquid_ratio}")),
        Line::from(""),
        Line::from(vec![
            Span::raw(format!("负债率: {}", fmt_percent(health.debt_ratio))),
            Span::raw("  |  "),
            Span::raw(format!("流动性: {}", fmt_percent(health.liquidity_ratio))),
            Span::raw("  |  "),
            Span::styled(
                format!("健康评分: {:.0}/100 [{}]", health.health_score, health.health_level),
                Style::default().fg(health_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("← 上月  |  下月 →", Style::default().fg(DARK))),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().fg(Color::White)),
        area,
    );
}

fn calc_growth_display(current: f64, previous: f64) -> (String, Color) {
    if previous.abs() < 1e-9 {
        return ("—".into(), DARK);
    }
    fmt_growth(Some((current - previous) / previous.abs()))
}

// ── 大额支出 ───────────────────────────────────────────────────────────────────

fn render_large_expenses(frame: &mut Frame, app: &App, area: Rect) {
    if app.adding_expense {
        render_expense_form(frame, app, area);
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 💸 大额支出记录（共 {} 条）[a]新增 [d]删除", app.large_expenses.len()))
        .title_style(title_style());

    if app.large_expenses.is_empty() {
        frame.render_widget(
            Paragraph::new("暂无大额支出记录，按 [a] 添加")
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(DARK)),
            area,
        );
        return;
    }

    let rows: Vec<Row> = app
        .large_expenses
        .iter()
        .map(|tx| {
            Row::new([
                Cell::from(tx.date.clone()),
                Cell::from(tx.category_name.clone().unwrap_or_else(|| "未分类".into())),
                Cell::from(Line::styled(
                    format!("-¥{:.2}", tx.amount),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ).alignment(Alignment::Right)),
                Cell::from(tx.account_name.clone().unwrap_or_default()),
                Cell::from(tx.note.clone().unwrap_or_default()),
            ])
        })
        .collect();

    let header = Row::new(["日期", "分类", "金额", "账户", "备注"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let widths = [
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Min(0),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(sel_style())
        .highlight_symbol("▶ ");

    let mut state = ratatui::widgets::TableState::default().with_selected(Some(app.expense_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_expense_form(frame: &mut Frame, app: &App, area: Rect) {
    let form_area = centered_rect(62, 75, area);
    frame.render_widget(Clear, form_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 💸 记录大额支出  [Ctrl+S]保存  [Esc]取消 ")
        .title_style(title_style());

    let inner = block.inner(form_area);
    frame.render_widget(block, form_area);

    let form = &app.expense_form;
    let fields_area = Layout::vertical([Constraint::Length(3); 5]).margin(1).split(inner);

    // 金额
    render_field(frame, fields_area[0], "金额 (¥)", &format!("{}_", form.amount),
        form.active_field == ExpenseField::Amount);

    // 分类
    let cat = form.categories.get(form.category_idx)
        .map(|c| format!("{} {} ↑↓", c.icon.as_deref().unwrap_or(""), c.name))
        .unwrap_or_else(|| "无分类".into());
    render_field(frame, fields_area[1], "分类", &cat, form.active_field == ExpenseField::Category);

    // 账户
    let acc = form.accounts.get(form.account_idx)
        .map(|a| format!("{} ↑↓", a.name))
        .unwrap_or_else(|| "无账户".into());
    render_field(frame, fields_area[2], "账户", &acc, form.active_field == ExpenseField::Account);

    // 日期
    render_field(frame, fields_area[3], "日期", &form.date, form.active_field == ExpenseField::Date);

    // 备注
    render_field(frame, fields_area[4], "备注", &form.note, form.active_field == ExpenseField::Note);
}

fn render_field(frame: &mut Frame, area: Rect, label: &str, value: &str, active: bool) {
    let border = if active { Style::default().fg(YELLOW) } else { Style::default().fg(DARK) };
    let title = if active { active_style() } else { Style::default().fg(Color::White) };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(format!(" {label} "), title))
        .border_style(border);
    frame.render_widget(Paragraph::new(value).block(block), area);
}

// ── 账户管理 ───────────────────────────────────────────────────────────────────

fn render_accounts(frame: &mut Frame, app: &App, area: Rect) {
    if app.adding_account {
        render_new_account_form(frame, app, area);
        return;
    }

    let total: f64 = app.accounts.iter().map(|a| a.balance).sum();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 💳 账户管理（共 {} 个，总计 {}）[a]新建 [d]删除", app.accounts.len(), fmt_balance(total)))
        .title_style(title_style());

    if app.accounts.is_empty() {
        frame.render_widget(
            Paragraph::new("暂无账户，按 [a] 新建").block(block).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|acc| {
            let color = if acc.balance >= 0.0 { GREEN } else { RED };
            Row::new([
                Cell::from(acc.name.clone()),
                Cell::from(acc.account_type.display_name()),
                Cell::from(acc.currency.clone()),
                Cell::from(Span::styled(fmt_balance(acc.balance), Style::default().fg(color))),
            ])
        })
        .collect();

    let header = Row::new(["账户名", "类型", "货币", "余额"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(40),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(sel_style())
        .highlight_symbol("▶ ");

    let mut state = ratatui::widgets::TableState::default().with_selected(Some(app.acc_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_new_account_form(frame: &mut Frame, app: &App, area: Rect) {
    let form_area = centered_rect(50, 40, area);
    frame.render_widget(Clear, form_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 新建账户  [Enter]确认  [Esc]取消 ")
        .title_style(title_style());
    let inner = block.inner(form_area);
    frame.render_widget(block, form_area);

    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Length(3), Constraint::Min(0)])
        .margin(1)
        .split(inner);

    render_field(frame, chunks[0], "账户名称", &format!("{}_", app.new_acc_name), true);

    use crate::models::account::AccountType;
    let types = AccountType::all();
    let current = &types[app.new_acc_type_idx % types.len()];
    render_field(frame, chunks[1], "类型 [←→切换]", current.display_name(), false);

    frame.render_widget(
        Paragraph::new("支持：现金/银行卡/微信/支付宝/股票/虚拟货币/社保/基金")
            .style(Style::default().fg(DARK)),
        chunks[2],
    );
}

// ── CSV 导入弹窗 ───────────────────────────────────────────────────────────────

fn render_import_modal(frame: &mut Frame, app: &App, area: Rect) {
    let modal_area = centered_rect(64, 50, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 📥 导入 CSV  [Ctrl+S]确认  [Tab]切换字段  [Esc]取消 ")
        .title_style(title_style());

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::vertical([
        Constraint::Length(3), // 文件路径
        Constraint::Length(3), // 账户选择
        Constraint::Min(0),    // 说明
    ])
    .margin(1)
    .split(inner);

    // 文件路径输入框
    render_field(
        frame,
        chunks[0],
        "CSV 文件路径",
        &format!("{}_", app.import_form.file_path),
        app.import_form.active_field == ImportField::FilePath,
    );

    // 账户选择器
    let acc_text = if app.accounts.is_empty() {
        "无账户，请先创建".into()
    } else {
        let a = &app.accounts[app.import_form.account_idx % app.accounts.len()];
        format!("◀ {} ({}) ▶", a.name, a.account_type.display_name())
    };
    render_field(
        frame,
        chunks[1],
        "导入到账户 [←→切换]",
        &acc_text,
        app.import_form.active_field == ImportField::Account,
    );

    // 格式说明
    let hint = Paragraph::new(
        "CSV 需包含表头：日期, 金额, 备注, 类型\n\
         类型值：收入/支出（留空默认为支出）\n\
         支持日期格式：YYYY-MM-DD / YYYY/MM/DD / YYYY年MM月DD日",
    )
    .style(Style::default().fg(DARK));
    frame.render_widget(hint, chunks[2]);
}

// ── 清空数据确认弹窗 ───────────────────────────────────────────────────────────

fn render_clear_confirm_modal(frame: &mut Frame, area: Rect) {
    let modal_area = centered_rect(60, 55, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RED))
        .title(Span::styled(
            " ⚠️  清空所有数据 ",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ));

    let text = vec![
        Line::from(""),
        Line::from(Span::styled("此操作将永久删除：", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("  • 所有账户（含余额记录）", Style::default().fg(YELLOW))),
        Line::from(Span::styled("  • 所有账目流水", Style::default().fg(YELLOW))),
        Line::from(Span::styled("  • 所有分类", Style::default().fg(YELLOW))),
        Line::from(Span::styled("  • 所有月结快照及历史", Style::default().fg(YELLOW))),
        Line::from(Span::styled("  • 所有分析聚合数据", Style::default().fg(YELLOW))),
        Line::from(""),
        Line::from(Span::styled("⚠️  操作不可撤销，建议先备份！", Style::default().fg(RED).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  按 ", Style::default().fg(DARK)),
            Span::styled("[y]", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Span::styled(" 确认清空    按 ", Style::default().fg(DARK)),
            Span::styled("[Esc/n]", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" 取消", Style::default().fg(DARK)),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, modal_area);
}

// ── 工具 ──────────────────────────────────────────────────────────────────────

fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - pct_y) / 2),
        Constraint::Percentage(pct_y),
        Constraint::Percentage((100 - pct_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - pct_x) / 2),
        Constraint::Percentage(pct_x),
        Constraint::Percentage((100 - pct_x) / 2),
    ])
    .split(v[1])[1]
}
