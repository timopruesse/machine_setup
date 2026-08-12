use std::io::IsTerminal;

use crossterm::style::Stylize;

use super::model::{CatalogItem, CatalogStatus};

const MAX_TITLE_LEN: usize = 40;

pub fn color_enabled() -> bool {
    color_enabled_with(
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    )
}

/// Returns whether ANSI styling should be applied for the given environment.
pub(crate) fn color_enabled_with(is_tty: bool, no_color_set: bool) -> bool {
    is_tty && !no_color_set
}

pub fn print_list(items: &[CatalogItem]) {
    print!("{}", render_list(items, color_enabled()));
}

/// Renders the catalog task list as plain text with width-aligned columns.
///
/// Under each row, only the `"Commands"` DetailSection lines are printed;
/// `"Meta"` / `"History"` sections are omitted here (they appear in the TUI detail
/// pane and column fields instead).
pub fn render_list(items: &[CatalogItem], color: bool) -> String {
    if items.is_empty() {
        return "No tasks defined.\n".to_string();
    }

    let installed_count = items
        .iter()
        .filter(|item| item.status == CatalogStatus::Installed)
        .count();

    let widths = column_widths(items);

    let mut out = format!(
        "Tasks  ({} total · {} installed)\n\n",
        items.len(),
        installed_count
    );

    for (index, item) in items.iter().enumerate() {
        render_item(&mut out, item, color, &widths);
        if index + 1 < items.len() {
            out.push('\n');
        }
    }

    out.push('\n');
    out
}

struct ColumnWidths {
    title: usize,
    os: usize,
    installed_at: usize,
    updated_at: usize,
}

fn column_widths(items: &[CatalogItem]) -> ColumnWidths {
    let mut widths = ColumnWidths {
        title: 0,
        os: 0,
        installed_at: 0,
        updated_at: 0,
    };

    for item in items {
        let title = truncate_title(&item.title);
        widths.title = widths.title.max(char_len(&title));
        widths.os = widths.os.max(char_len(&item.os_label));
        widths.installed_at = widths.installed_at.max(char_len(&item.installed_at));
        widths.updated_at = widths.updated_at.max(char_len(&item.updated_at));
    }

    widths
}

fn render_item(out: &mut String, item: &CatalogItem, color: bool, widths: &ColumnWidths) {
    let glyph = status_glyph(&item.status);
    let styled_glyph = style_glyph(glyph, &item.status, color);
    let title = pad_field(&truncate_title(&item.title), widths.title);
    let os = pad_field(&item.os_label, widths.os);
    let installed_at = pad_field(&item.installed_at, widths.installed_at);
    let updated_at = pad_field(&item.updated_at, widths.updated_at);
    let badges = item.badges.join(" ");

    out.push_str(&styled_glyph);
    out.push(' ');
    out.push_str(&title);
    out.push(' ');
    out.push_str(&os);
    out.push(' ');
    out.push_str(&installed_at);
    out.push(' ');
    out.push_str(&updated_at);
    if !badges.is_empty() {
        out.push(' ');
        out.push_str(&badges);
    }
    out.push('\n');

    if let Some(section) = item
        .detail
        .iter()
        .find(|section| section.title == "Commands")
    {
        for line in &section.lines {
            out.push_str("    ");
            out.push_str(&style_muted(line, color));
            out.push('\n');
        }
    }
}

fn status_glyph(status: &CatalogStatus) -> &'static str {
    match status {
        CatalogStatus::Installed => "✓",
        CatalogStatus::NotInstalled | CatalogStatus::Neutral => "·",
        CatalogStatus::SkippedOs => "–",
    }
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_LEN {
        return title.to_string();
    }

    let truncated: String = title.chars().take(MAX_TITLE_LEN - 1).collect();
    format!("{truncated}…")
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn pad_field(s: &str, width: usize) -> String {
    let len = char_len(s);
    if len >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn style_glyph(glyph: &str, status: &CatalogStatus, color: bool) -> String {
    if !color {
        return glyph.to_string();
    }

    match status {
        CatalogStatus::Installed => glyph.green().to_string(),
        CatalogStatus::SkippedOs => glyph.yellow().to_string(),
        CatalogStatus::NotInstalled | CatalogStatus::Neutral => glyph.dark_grey().to_string(),
    }
}

fn style_muted(text: &str, color: bool) -> String {
    if color {
        text.dark_grey().to_string()
    } else {
        text.to_string()
    }
}

/// Pretty plain doctor report: task table + validation + orphans.
pub fn print_doctor(items: &[CatalogItem], issue_lines: &[String], orphans: &[String]) {
    print!(
        "{}",
        render_doctor(items, issue_lines, orphans, color_enabled())
    );
}

pub fn render_doctor(
    items: &[CatalogItem],
    issue_lines: &[String],
    orphans: &[String],
    color: bool,
) -> String {
    let mut out = render_list(items, color);

    out.push_str("Validation:\n");
    if issue_lines.is_empty() {
        out.push_str("  Config is valid.\n");
    } else {
        for line in issue_lines {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("\nHistory orphans:\n");
    if orphans.is_empty() {
        out.push_str("  none\n");
    } else {
        for name in orphans {
            out.push_str("  ");
            out.push_str(name);
            out.push_str(" (in History, not in Config document)\n");
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::catalog::model::DetailSection;

    fn sample_item() -> CatalogItem {
        CatalogItem {
            id: "dotfiles".into(),
            title: "dotfiles".into(),
            status: CatalogStatus::Installed,
            os_label: "all".into(),
            installed_at: "2026-08-01".into(),
            updated_at: "-".into(),
            badges: vec!["parallel".into()],
            detail: vec![DetailSection {
                title: "Commands".into(),
                lines: vec!["- run: echo hello".into()],
            }],
        }
    }

    fn item_rows(out: &str) -> Vec<&str> {
        out.lines()
            .filter(|line| line.starts_with('✓') || line.starts_with('·') || line.starts_with('–'))
            .collect()
    }

    #[test]
    fn render_list_shows_glyph_name_and_commands() {
        let items = vec![sample_item()];
        let out = render_list(&items, false);

        assert!(out.contains('✓'));
        assert!(out.contains("dotfiles"));
        assert!(out.contains("- run: echo hello"));
    }

    #[test]
    fn render_list_shows_not_installed_and_skipped_glyphs() {
        let mut pending = sample_item();
        pending.title = "pending".into();
        pending.status = CatalogStatus::NotInstalled;
        pending.badges.clear();
        pending.detail.clear();

        let mut skipped = sample_item();
        skipped.title = "windows-only".into();
        skipped.status = CatalogStatus::SkippedOs;
        skipped.os_label = "windows".into();
        skipped.badges = vec!["os skip".into()];
        skipped.detail.clear();

        let out = render_list(&[pending, skipped], false);

        assert!(out.contains('·'));
        assert!(out.contains('–'));
        assert!(out.contains("pending"));
        assert!(out.contains("windows-only"));
        assert!(out.contains("os skip"));
    }

    #[test]
    fn render_list_empty_shows_no_tasks_or_zero() {
        let out = render_list(&[], false);
        assert!(out.contains("No tasks") || out.contains('0'));
    }

    #[test]
    fn render_list_header_counts() {
        let mut pending = sample_item();
        pending.title = "pending".into();
        pending.status = CatalogStatus::NotInstalled;
        pending.badges.clear();

        let items = vec![sample_item(), pending];
        let out = render_list(&items, false);

        assert!(out.contains("2 total"));
        assert!(out.contains("1 installed"));
    }

    #[test]
    fn render_list_truncates_long_title_with_ellipsis() {
        let mut item = sample_item();
        item.title = "a".repeat(50);
        item.detail.clear();

        let out = render_list(&[item], false);

        let expected = truncate_title(&"a".repeat(50));
        assert!(expected.ends_with('…'));
        assert!(out.contains(&expected));
        assert!(!out.contains(&"a".repeat(50)));
    }

    #[test]
    fn render_list_aligns_columns_across_differing_title_lengths() {
        let mut short = sample_item();
        short.title = "ab".into();
        short.status = CatalogStatus::NotInstalled;
        short.badges.clear();
        short.detail.clear();

        let mut long = sample_item();
        long.title = "abcdefghij".into();
        long.status = CatalogStatus::NotInstalled;
        long.badges.clear();
        long.detail.clear();

        let out = render_list(&[short, long], false);
        let rows = item_rows(&out);
        assert_eq!(rows.len(), 2);

        let os0 = rows[0].find(" all ").expect("os column on short row");
        let os1 = rows[1].find(" all ").expect("os column on long row");
        assert_eq!(os0, os1, "os column should share the same start index");

        let installed0 = rows[0]
            .find("2026-08-01")
            .expect("installed_at on short row");
        let installed1 = rows[1]
            .find("2026-08-01")
            .expect("installed_at on long row");
        assert_eq!(
            installed0, installed1,
            "installed_at column should share the same start index"
        );

        // Title padding: short name is followed by pad + separator spaces before os.
        assert!(
            rows[0].contains("ab         all"),
            "short title should be right-padded; got {:?}",
            rows[0]
        );
    }

    #[test]
    fn render_list_aligns_os_and_timestamp_columns() {
        let mut short_os = sample_item();
        short_os.title = "task-a".into();
        short_os.status = CatalogStatus::NotInstalled;
        short_os.os_label = "all".into();
        short_os.installed_at = "-".into();
        short_os.updated_at = "-".into();
        short_os.badges.clear();
        short_os.detail.clear();

        let mut long_os = sample_item();
        long_os.title = "task-b".into();
        long_os.status = CatalogStatus::NotInstalled;
        long_os.os_label = "linux, macos".into();
        long_os.installed_at = "2026-08-01 12:00 UTC".into();
        long_os.updated_at = "2026-08-12 09:30 UTC".into();
        long_os.badges.clear();
        long_os.detail.clear();

        let out = render_list(&[short_os, long_os], false);
        let rows = item_rows(&out);
        assert_eq!(rows.len(), 2);

        // Same-length titles + same glyph → OS column starts at the same byte index.
        let os0 = rows[0].find("all").expect("short os label");
        let os1 = rows[1].find("linux, macos").expect("long os label");
        assert_eq!(os0, os1);

        // installed_at / updated_at share column starts despite "-" vs full timestamps.
        let inst_marker = "2026-08-01 12:00 UTC";
        let inst1 = rows[1].find(inst_marker).expect("installed_at on long row");
        assert_eq!(
            &rows[0][inst1..inst1 + 1],
            "-",
            "short installed_at should sit at the long timestamp column"
        );

        let upd_marker = "2026-08-12 09:30 UTC";
        let upd1 = rows[1].find(upd_marker).expect("updated_at on long row");
        assert_eq!(
            &rows[0][upd1..upd1 + 1],
            "-",
            "short updated_at should sit at the long timestamp column"
        );

        // Padded gap between short os and installed_at: spaces fill to long os width.
        assert!(
            rows[0][os0 + 3..inst1].chars().all(|c| c == ' '),
            "os column should be right-padded with spaces"
        );
    }

    #[test]
    fn color_enabled_with_respects_tty_and_no_color() {
        assert!(color_enabled_with(true, false));
        assert!(!color_enabled_with(true, true));
        assert!(!color_enabled_with(false, false));
        assert!(!color_enabled_with(false, true));
    }

    #[test]
    fn render_doctor_includes_validation_and_orphans() {
        let items = vec![sample_item()];
        let issues = vec!["[WARN] empty: no commands".into()];
        let orphans = vec!["gone".into()];
        let out = render_doctor(&items, &issues, &orphans, false);

        assert!(out.contains("Validation:"));
        assert!(out.contains("[WARN] empty: no commands"));
        assert!(out.contains("History orphans:"));
        assert!(out.contains("gone"));
        assert!(out.contains("dotfiles"));
    }
}
