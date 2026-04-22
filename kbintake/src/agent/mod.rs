pub mod scheduler;
pub mod worker;

use anyhow::Result;

use crate::app::App;

pub fn run_agent(app: &App) -> Result<()> {
    scheduler::drain_queue(app)
}
