pub mod app;
pub mod plain;
pub mod widgets;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::engine::event::TaskEvent;
use app::App;

/// Restore the terminal to its normal state.
/// Safe to call multiple times.
fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
}

/// Run the TUI, consuming events from the engine until all tasks are done.
pub async fn run(
    mut event_rx: mpsc::UnboundedReceiver<TaskEvent>,
    task_names: Vec<String>,
    mode: crate::cli::Command,
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

    let mut app = App::new(task_names, mode);

    let result = run_loop(&mut terminal, &mut app, &mut event_rx, &cancel).await;

    // Always restore terminal
    restore_terminal();
    terminal.show_cursor()?;

    // Print final summary to stdout
    print_summary(&app);

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_rx: &mut mpsc::UnboundedReceiver<TaskEvent>,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    loop {
        // Drain all pending task events (non-blocking)
        loop {
            match event_rx.try_recv() {
                Ok(task_event) => app.handle_event(task_event),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    app.done = true;
                    break;
                }
            }
        }

        // Render
        terminal.draw(|f| render(f, app))?;

        // Handle keyboard input (with timeout for responsive UI)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        cancel.cancel();
                        return Ok(());
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        cancel.cancel();
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
                    KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                    KeyCode::PageUp => app.scroll_log_up(),
                    KeyCode::PageDown => app.scroll_log_down(),
                    KeyCode::Home => app.scroll_log_to_top(),
                    KeyCode::End => app.scroll_log_to_bottom(),
                    _ => {}
                }
            }
        }
    }
}

fn render(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header/progress bar
            Constraint::Min(5),    // Main content
            Constraint::Length(1), // Help bar
        ])
        .split(f.area());

    widgets::header::render(f, chunks[0], app);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(chunks[1]);

    widgets::task_list::render(f, main_chunks[0], app);
    widgets::log_view::render(f, main_chunks[1], app);
    widgets::help_bar::render(f, chunks[2], app);
}

fn print_summary(app: &App) {
    println!(
        "\nmachine_setup {}: {} succeeded, {} failed, {} skipped\n",
        app.mode, app.succeeded, app.failed, app.skipped
    );

    for task in &app.tasks {
        if let app::TaskStatus::Failed(ref error) = task.status {
            println!("  FAILED: {} - {}", task.name, error);
        }
    }
}
