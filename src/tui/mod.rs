mod app;
mod events;
mod ui;

use std::sync::Arc;

use crate::db::DbPool;
use crate::error::Result;
use app::App;

pub fn run_tui(pool: Arc<DbPool>) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new(pool)?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
