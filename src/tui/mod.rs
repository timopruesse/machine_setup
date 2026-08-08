pub mod event_loop;
pub mod format;
pub mod message;
pub mod plain;
pub mod reduce;
pub mod state;
pub mod widgets;

use std::io;

use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::engine::event::TaskEvent;
use state::{TaskStatus, UiState};

/// Restore the terminal to its normal state.
/// Safe to call multiple times.
pub(crate) fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
}

/// Run the TUI, consuming events from the engine until all tasks are done.
pub async fn run(
    event_rx: mpsc::UnboundedReceiver<TaskEvent>,
    task_names: Vec<String>,
    mode: crate::engine::mode::Mode,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    // Install panic hook that restores the terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));

    // Set up terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let state = UiState::new(task_names, mode);

    let result = event_loop::run_loop(&mut terminal, state, event_rx, cancel).await;

    // Always restore terminal
    restore_terminal();
    let _ = terminal.show_cursor();

    match result {
        Ok(final_state) => {
            print_summary(&final_state);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn render(f: &mut ratatui::Frame, state: &UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/progress bar
            Constraint::Min(5),    // Main content
            Constraint::Length(1), // Help bar
        ])
        .split(f.area());

    widgets::header::render(f, chunks[0], state);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    widgets::task_list::render(f, main_chunks[0], state);
    widgets::log_view::render(f, main_chunks[1], state);
    widgets::help_bar::render(f, chunks[2], state);
}

fn print_summary(state: &UiState) {
    let elapsed = state
        .run_elapsed
        .map(crate::tui::format::format_duration)
        .map(|s| format!(" in {s}"))
        .unwrap_or_default();

    println!(
        "\nmachine_setup {}: {} succeeded, {} failed, {} skipped{elapsed}\n",
        state.mode, state.succeeded, state.failed, state.skipped
    );

    for task in &state.tasks {
        if let TaskStatus::Failed(ref error) = task.status {
            println!("  FAILED: {} - {}", task.name, error);
        }
    }
}
