pub mod catalog;
pub mod details;
pub mod event_loop;
pub mod format;
pub mod log_display;
pub mod message;
pub mod parallel_burst;
pub mod plain;
pub mod reduce;
pub mod state;
pub mod theme;
pub mod widgets;

use std::io;

use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::engine::event::TaskEvent;
use state::{TaskStatus, UiState};
use theme::{Theme, DETAILS_MIN_WIDTH, MIN_USABLE_HEIGHT};

/// Restore the terminal to its normal state.
/// Safe to call multiple times.
pub(crate) fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
}

/// Run the TUI, consuming events from the engine until all tasks are done.
pub async fn run(
    event_rx: mpsc::Receiver<TaskEvent>,
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

    let result = event_loop::run_loop(&mut terminal, state, event_rx, cancel.clone()).await;

    // Always restore terminal
    restore_terminal();
    let _ = terminal.show_cursor();

    match result {
        Ok(final_state) => {
            print_summary(&final_state);
            if cancel.is_cancelled() {
                anyhow::bail!("execution was cancelled");
            }
            if final_state.failed > 0 {
                anyhow::bail!("{} task(s) failed", final_state.failed);
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn should_collapse_details(main_width: u16) -> bool {
    main_width < DETAILS_MIN_WIDTH
}

pub(crate) fn render(f: &mut ratatui::Frame, state: &UiState, theme: &Theme) {
    let area = f.area();

    if area.height < MIN_USABLE_HEIGHT {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        if chunks[0].width > 0 && chunks[0].height > 0 {
            let msg = Paragraph::new(Line::from(Span::styled(
                " terminal too small ",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            )));
            f.render_widget(msg, chunks[0]);
        }

        widgets::help_bar::render(f, chunks[1], state, theme);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/progress bar
            Constraint::Min(5),    // Main content
            Constraint::Length(1), // Help bar
        ])
        .split(area);

    widgets::header::render(f, chunks[0], state, theme);

    let main_area = chunks[1];
    if should_collapse_details(main_area.width) {
        widgets::task_list::render(f, main_area, state, theme);
    } else {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(main_area);

        widgets::task_list::render(f, main_chunks[0], state, theme);
        widgets::log_view::render(f, main_chunks[1], state, theme);
    }

    widgets::help_bar::render(f, chunks[2], state, theme);
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

#[cfg(test)]
mod tests {
    #[test]
    fn collapse_details_when_main_narrow() {
        assert!(crate::tui::should_collapse_details(67));
        assert!(!crate::tui::should_collapse_details(68));
    }
}
