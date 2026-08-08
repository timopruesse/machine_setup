use std::time::Duration;

use ratatui::style::Color;

use crate::tui::state::TASK_PALETTE_LEN;

/// Accent color for a task's list row / merge prefix.
pub fn task_palette_color(color_idx: usize) -> Color {
    const COLORS: [Color; TASK_PALETTE_LEN] = [
        Color::Cyan,
        Color::Magenta,
        Color::Blue,
        Color::Green,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
    ];
    COLORS[color_idx % TASK_PALETTE_LEN]
}

/// Format a duration for the TUI (compact, fixed-ish width feel).
///
/// - under 10s → one decimal (`3.4s`)
/// - under 60s → whole seconds (`12s`)
/// - under 1h → `1m 05s`
/// - otherwise → `1h 02m`
pub fn format_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms < 10_000 {
        let secs = total_ms as f64 / 1000.0;
        format!("{secs:.1}s")
    } else if total_ms < 60_000 {
        format!("{}s", total_ms / 1000)
    } else {
        let total_secs = d.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if hours > 0 {
            format!("{hours}h {mins:02}m")
        } else {
            format!("{mins}m {secs:02}s")
        }
    }
}

/// Live or frozen task duration for display.
pub fn task_elapsed(
    started_at: Option<std::time::Instant>,
    duration: Option<Duration>,
) -> Option<Duration> {
    duration.or_else(|| started_at.map(|t| t.elapsed()))
}

/// Run elapsed: frozen when done, otherwise live from start.
pub fn run_elapsed(run_started: std::time::Instant, run_elapsed: Option<Duration>) -> Duration {
    run_elapsed.unwrap_or_else(|| run_started.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_under_ten_seconds_one_decimal() {
        assert_eq!(format_duration(Duration::from_millis(3400)), "3.4s");
        assert_eq!(format_duration(Duration::from_millis(0)), "0.0s");
    }

    #[test]
    fn format_under_minute_whole_seconds() {
        assert_eq!(format_duration(Duration::from_secs(12)), "12s");
        assert_eq!(format_duration(Duration::from_millis(59_999)), "59s");
    }

    #[test]
    fn format_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 05s");
        assert_eq!(format_duration(Duration::from_secs(600)), "10m 00s");
    }

    #[test]
    fn format_hours_and_minutes() {
        assert_eq!(format_duration(Duration::from_secs(3725)), "1h 02m");
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h 00m");
    }
}
