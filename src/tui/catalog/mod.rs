pub mod adapt;
pub mod event_loop;
pub mod message;
pub mod model;
pub mod plain;
pub mod reduce;
pub mod state;
pub mod view;

use std::io;

use crossterm::terminal::{self, EnterAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

pub use message::{CatalogEffect, CatalogInput, CatalogMessage};
pub use model::{CatalogItem, CatalogMode, CatalogStatus, DetailSection};
pub use state::CatalogState;

use event_loop::{run as run_event_loop, LoopOutcome};

/// Browse the catalog in a master–detail TUI. Empty lists print a one-liner and return.
pub fn run_browse(items: Vec<CatalogItem>) -> anyhow::Result<()> {
    if items.is_empty() {
        println!("No tasks defined.");
        return Ok(());
    }

    with_terminal(CatalogMode::Browse, items, |outcome| match outcome {
        LoopOutcome::Quit | LoopOutcome::Abort | LoopOutcome::Confirm(_) => Ok(()),
    })
}

/// Multi-select catalog TUI. Returns selected task ids, or `None` on abort / empty input.
pub fn run_select(items: Vec<CatalogItem>) -> anyhow::Result<Option<Vec<String>>> {
    if items.is_empty() {
        return Ok(None);
    }

    with_terminal(CatalogMode::Select, items, |outcome| match outcome {
        LoopOutcome::Confirm(ids) => Ok(Some(ids)),
        LoopOutcome::Quit | LoopOutcome::Abort => Ok(None),
    })
}

fn with_terminal<F, T>(mode: CatalogMode, items: Vec<CatalogItem>, f: F) -> anyhow::Result<T>
where
    F: FnOnce(LoopOutcome) -> anyhow::Result<T>,
{
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::tui::restore_terminal();
        original_hook(info);
    }));

    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let state = CatalogState::new(items, mode);
    let result = run_event_loop(&mut terminal, state);

    crate::tui::restore_terminal();
    let _ = terminal.show_cursor();

    match result {
        Ok(outcome) => f(outcome),
        Err(e) => Err(e),
    }
}
