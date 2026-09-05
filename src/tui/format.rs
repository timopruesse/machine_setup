use std::time::Duration;

use ratatui::style::Color;

use crate::tui::theme::Theme;

/// Strip ANSI escape sequences from command output before TUI display.
///
/// Ratatui places text into cells by visible width; ESC (`\x1b`) is dropped as
/// zero-width, which leaves CSI leftovers like `[33m` visible in the log panel.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if c == '\t' || !c.is_control() {
                out.push(c);
            }
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                // CSI: ESC [ ... final byte in 0x40..=0x7E
                chars.next();
                for nc in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&nc) {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC: ESC ] ... BEL or ST (ESC \)
                chars.next();
                while let Some(nc) = chars.next() {
                    if nc == '\u{07}' {
                        break;
                    }
                    if nc == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            Some(_) => {
                // Other 2-byte escapes (e.g. ESC c)
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// Accent color for a task's list row / merge prefix.
pub fn task_palette_color(theme: &Theme, color_idx: usize) -> Color {
    theme.task_color(color_idx)
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
    fn strip_ansi_removes_sgr_leaving_plain_text() {
        // Colored brew/plugin lines look like this when ESC is swallowed by ratatui:
        // `[1m[33mzsh-users/zsh-autosuggestions:[39m[0m`
        let raw = "\u{1b}[1m\u{1b}[33mzsh-users/zsh-autosuggestions:\u{1b}[39m\u{1b}[0m";
        assert_eq!(strip_ansi(raw), "zsh-users/zsh-autosuggestions:");
    }

    #[test]
    fn strip_ansi_preserves_plain_text() {
        assert_eq!(strip_ansi("  [done] install"), "  [done] install");
    }

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
