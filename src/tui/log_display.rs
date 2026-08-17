//! Log line filtering and styling for the Details pane.

use crate::engine::output::OutputKind;
use crate::tui::details::DetailsMode;
use crate::tui::state::{LogLine, TaskState};

/// Lines to render in the details pane (may omit redundant lifecycle rows).
pub fn display_lines(
    task: &TaskState,
    mode: DetailsMode,
    hide_command_start: bool,
) -> Vec<&LogLine> {
    task.log_lines
        .iter()
        .filter(|line| should_show_line(line, task, mode, hide_command_start))
        .collect()
}

fn should_show_line(
    line: &LogLine,
    task: &TaskState,
    mode: DetailsMode,
    hide_command_start: bool,
) -> bool {
    match line.kind {
        OutputKind::CommandDone => false,
        OutputKind::CommandStart if hide_command_start && task.status.is_running() => false,
        OutputKind::CommandStart
            if matches!(mode, DetailsMode::RunnerGrid | DetailsMode::ExpandedTask)
                && task.status.is_running() =>
        {
            false
        }
        _ => true,
    }
}

pub fn style_for_kind(kind: OutputKind) -> ratatui::style::Style {
    use ratatui::style::{Color, Modifier, Style};
    match kind {
        OutputKind::CommandStart => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        OutputKind::CommandDone => Style::default().fg(Color::Green),
        OutputKind::CommandFailed => Style::default().fg(Color::Red),
        OutputKind::Progress => Style::default().fg(Color::White),
        OutputKind::Subprocess => Style::default().fg(Color::DarkGray),
        OutputKind::SubprocessErr => Style::default().fg(Color::Yellow),
        OutputKind::Info => Style::default().fg(Color::Cyan),
        OutputKind::TaskStatus => Style::default().fg(Color::DarkGray),
    }
}

pub fn plain_prefix(kind: OutputKind) -> &'static str {
    match kind {
        OutputKind::SubprocessErr => "[stderr] ",
        OutputKind::Progress => "· ",
        OutputKind::Info => "· ",
        _ => "  ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::output::OutputKind;
    use crate::tui::state::TaskStatus;

    #[test]
    fn hides_command_done_lines() {
        let mut task = TaskState::new("a".into());
        task.push_log(OutputKind::Progress, "copy a → b".into());
        task.push_log(OutputKind::CommandDone, "done".into());
        let shown = display_lines(&task, DetailsMode::SingleTask, false);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].kind, OutputKind::Progress);
    }

    #[test]
    fn hides_command_start_in_runner_grid() {
        let mut task = TaskState::new("a".into());
        task.status = TaskStatus::Running;
        task.push_log(OutputKind::CommandStart, "copy …".into());
        task.push_log(OutputKind::Progress, "copy a → b".into());
        let shown = display_lines(&task, DetailsMode::RunnerGrid, true);
        assert!(shown.iter().all(|l| l.kind != OutputKind::CommandStart));
    }
}
