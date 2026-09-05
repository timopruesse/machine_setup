# TUI SilkCircuit Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply SilkCircuit Neon theme tokens and targeted UX polish to the run dashboard and catalog TUIs without changing panel layouts.

**Architecture:** Add `theme.rs` (neon/mono, resolved once at TUI start) and shared `widgets/chrome.rs` (rounded blocks, key hints). Thread `&Theme` through render paths. Layout gains narrow Details collapse and a height floor; help bar advertises real Esc/q semantics.

**Tech Stack:** Rust, ratatui, crossterm, existing `UiState` / `CatalogState` / `reduce`.

**Spec:** `docs/superpowers/specs/2026-09-05-tui-silkcircuit-polish-design.md`

## Global Constraints

- Do not change `TaskEvent`, engine runner/sink, or plain-mode redesign.
- Same panel skeletons: Header / Tasks|Details / Help (run); banner / Tasks|Detail / Help (catalog).
- Runner-grid max remains 4 bands.
- `DETAILS_MIN_WIDTH = 68`; height floor `< 8` rows → Help + size message.
- Completion strip in header gauge label: `N ok · M failed · elapsed` (omit failed clause when 0).
- `NO_COLOR` non-empty → `Theme::mono()` before RGB.
- No pluggable theme engine / mouse / multi-pane focus.
- Do **not** `git commit` in worker tasks — leave a clean diff for `/land` or committer when the user asks.
- `make check && make test && make lint` must stay green.

## File map

| File | Responsibility |
| --- | --- |
| `src/tui/theme.rs` | `Theme` slots, `neon`/`mono`/`resolve`, task palette, layout constants |
| `src/tui/widgets/chrome.rs` | `rounded_block`, `key_hint`, separator span |
| `src/tui/widgets/mod.rs` | export `chrome` |
| `src/tui/mod.rs` | `mod theme`; layout collapse; pass `&Theme` |
| `src/tui/event_loop.rs` | resolve theme once; pass to `render` |
| `src/tui/format.rs` | `task_palette_color` takes theme (or thin wrapper) |
| `src/tui/log_display.rs` | styles from theme |
| `src/tui/widgets/{header,help_bar,task_list}.rs` | themed render + UX |
| `src/tui/details/render.rs` | themed borders/status |
| `src/tui/widgets/log_view.rs` | forward theme |
| `src/tui/catalog/{mod,event_loop,view}.rs` | shared theme + chrome |
| `README.md` | keybinding / parallel UI note if stale |

---

### Task 1: Theme module + unit tests

**Files:**
- Create: `src/tui/theme.rs`
- Modify: `src/tui/mod.rs` — add `pub mod theme;`
- Modify: `src/tui/format.rs` — palette via theme

**Interfaces:**
- Produces:
  - `pub const DETAILS_MIN_WIDTH: u16 = 68;`
  - `pub const MIN_USABLE_HEIGHT: u16 = 8;`
  - `pub struct Theme { /* Copy+Clone */ accent, accent_alt, success, error, warning, info, muted, text, border, border_focus, gauge_bg, task_palette: [Color; TASK_PALETTE_LEN] }`
  - `Theme::neon() -> Self`
  - `Theme::mono() -> Self` — named ANSI approximating slots (Cyan/Magenta/Green/Red/Yellow/DarkGray/White)
  - `Theme::resolve() -> Self` — if `std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())` then mono else neon
  - `Theme::task_color(&self, idx: usize) -> Color`
  - Neon hex → `Color::Rgb`: accent `#e135ff`, accent_alt/info `#80ffea`, success `#50fa7b`, error `#ff6363`, warning `#f1fa8c`, muted `#82879f`, text `#f8f8f2`, border `#3c3c50`, border_focus `#e135ff`, gauge_bg `#37324b`
  - Task palette neon (8): `#e135ff`, `#80ffea`, `#ff6ac1`, `#50fa7b`, `#f1fa8c`, `#ff55ff`, `#bd93f9`, `#ff99ff`
- Consumes: `crate::tui::state::TASK_PALETTE_LEN`

- [ ] **Step 1: Write failing tests** in `theme.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neon_accent_is_electric_purple() {
        let t = Theme::neon();
        assert_eq!(t.accent, Color::Rgb(0xe1, 0x35, 0xff));
    }

    #[test]
    fn resolve_respects_nonempty_no_color() {
        // Use a scoped env mutation or test mono path directly if env is flaky in parallel:
        let mono = Theme::mono();
        assert_eq!(mono.accent, Color::Magenta); // or Cyan — pick one and document
    }

    #[test]
    fn task_color_wraps_palette() {
        let t = Theme::neon();
        assert_eq!(t.task_color(0), t.task_palette[0]);
        assert_eq!(t.task_color(TASK_PALETTE_LEN), t.task_palette[0]);
    }
}
```

For `resolve_respects_nonempty_no_color`, prefer testing `Theme::mono()` equality of semantic slots rather than mutating process env if the suite runs parallel; add a separate test that documents `resolve` logic with a small `fn should_use_mono(no_color: Option<&OsStr>) -> bool` used by `resolve`.

- [ ] **Step 2: Implement `theme.rs` and wire `mod theme`.**
- [ ] **Step 3: Change `format::task_palette_color` to:**

```rust
pub fn task_palette_color(theme: &Theme, color_idx: usize) -> Color {
    theme.task_color(color_idx)
}
```

Update all call sites in later tasks (temporarily keep a deprecated path only if needed to compile mid-work — prefer fixing call sites in Task 3/4 in the same branch).

- [ ] **Step 4: Run** `cargo test theme:: --lib` — expect PASS.
- [ ] **Step 5:** Do not commit.

---

### Task 2: Chrome helpers + help-bar discoverability

**Files:**
- Create: `src/tui/widgets/chrome.rs`
- Modify: `src/tui/widgets/mod.rs`
- Modify: `src/tui/widgets/help_bar.rs`

**Interfaces:**
- Consumes: `Theme` from Task 1
- Produces:
  - `pub fn key_hint(theme: &Theme, key: &str, action: &str) -> Vec<Span<'static>>` — key = `theme.accent` + BOLD; action = `theme.muted`
  - `pub fn hint_separator(theme: &Theme) -> Span<'static>` — `" · "` in `muted`
  - `pub fn rounded_block(theme: &Theme, focused: bool) -> Block<'static>` — `Borders::ALL`, `BorderType::Rounded`, border fg = `border_focus` if focused else `border`
  - `help_bar::render(f, area, state, theme)`
  - Help content rules:
    - `search_mode`: Esc cancel, Enter apply, j/k navigate
    - `filter_active` (not search): Esc clear filter, q cancel+quit (label `"cancel"` while `!done`, `"quit"` when done), j/k, /
    - `done` and no filter: **Esc quit**, q quit, j/k, /
    - running: q **cancel**, burst Enter expand/collapse, j/k band|navigate, PgUp/PgDn, Home/End, /
    - if `!log_follow && !search_mode`: append `End follow`

- [ ] **Step 1: Add `chrome.rs` + export; rewrite `help_bar` to use theme + rules above.**
- [ ] **Step 2: Unit-test hint selection** — extract `pub(crate) fn help_hints(state: &UiState) -> Vec<(&'static str, &'static str)>` (key, action) and test:

```rust
#[test]
fn done_idle_advertises_esc_quit() {
    let mut state = UiState::new(vec!["a".into()], Mode::Install);
    state.done = true;
    let hints = help_hints(&state);
    assert!(hints.iter().any(|(k, a)| *k == "Esc" && *a == "quit"));
}

#[test]
fn running_advertises_q_cancel() {
    let state = UiState::new(vec!["a".into()], Mode::Install);
    let hints = help_hints(&state);
    assert!(hints.iter().any(|(k, a)| *k == "q" && *a == "cancel"));
}
```

(Adjust `UiState` construction to match existing test helpers in `reduce.rs`.)

- [ ] **Step 3: Run** `cargo test help_bar:: --lib` and `cargo test chrome:: --lib` if any.
- [ ] **Step 4:** Do not commit. Temporary compile breakage in `mod::render` / catalog is OK until Task 3/4 if you stub theme param — prefer finishing Task 3 in the same worker session if needed to keep `cargo check` green.

---

### Task 3: Run dashboard theming + narrow layout + completion strip

**Files:**
- Modify: `src/tui/mod.rs` — `render(f, state, theme)`; collapse logic
- Modify: `src/tui/event_loop.rs` — `let theme = Theme::resolve();` pass through
- Modify: `src/tui/widgets/header.rs`, `task_list.rs`, `log_view.rs`
- Modify: `src/tui/details/render.rs`
- Modify: `src/tui/log_display.rs`
- Modify: `src/tui/format.rs` call sites

**Interfaces:**
- Consumes: Theme, chrome
- Produces:
  - `pub(crate) fn split_main(area: Rect) -> MainLayout` or inline in `render`:
    - if `f.area().height < MIN_USABLE_HEIGHT`: draw only help area + `Paragraph` “terminal too small” in remaining; return early
    - else vertical: header 3 / min 5 / help 1
    - if `chunks[1].width < DETAILS_MIN_WIDTH`: tasks full width; skip details
    - else 30%/70% split as today
  - Header completion when `done`: label ` {succeeded} ok · {failed} failed · {elapsed} ` (if `failed == 0`: `{succeeded} ok · {skipped} skipped · {elapsed}` or include skipped always — **lock:** `"{succeeded} ok · {failed} failed · {elapsed}"` and if failed==0 use `"{succeeded} ok · {elapsed}"` optionally appending skipped if >0 as ` · {skipped} skipped`)
  - Spec success criteria: `N ok · M failed · elapsed` — implement:

```rust
if state.failed > 0 {
    format!(" {} ok · {} failed · {elapsed} ", state.succeeded, state.failed)
} else if state.skipped > 0 {
    format!(" {} ok · {} skipped · {elapsed} ", state.succeeded, state.skipped)
} else {
    format!(" {} ok · {elapsed} ", state.succeeded)
}
```

  - Header colors from theme (`success`/`error`/`accent_alt` while running); gauge bg `gauge_bg`; rounded border; title `text` bold
  - Task list / details: `rounded_block`, status colors from theme, `task_palette_color(theme, idx)`
  - `log_display::style_for(kind, theme) -> Style`
  - Zero-area: if `area.width == 0 || area.height == 0` return early from widget renders

- [ ] **Step 1: Implement layout + wire theme through all run render functions.**
- [ ] **Step 2: Add unit test for layout decision:**

```rust
#[test]
fn collapse_details_when_main_narrow() {
    assert!(crate::tui::should_collapse_details(67));
    assert!(!crate::tui::should_collapse_details(68));
}
```

Expose `pub(crate) fn should_collapse_details(main_width: u16) -> bool { main_width < DETAILS_MIN_WIDTH }`.

- [ ] **Step 3: Run** `cargo test --lib` focused on tui; then `cargo check`.
- [ ] **Step 4:** Do not commit.

---

### Task 4: Catalog shared theme + chrome

**Files:**
- Modify: `src/tui/catalog/event_loop.rs` — resolve theme once (or accept from caller)
- Modify: `src/tui/catalog/view.rs` — `render(f, state, theme)`; replace local `key_hint` with `widgets::chrome`; rounded borders; semantic colors from theme
- Modify: `src/tui/catalog/mod.rs` if needed for exports

**Interfaces:**
- Consumes: `Theme`, `chrome::key_hint`, `chrome::rounded_block`
- Produces: catalog Browse/Select visually aligned with run dashboard

- [ ] **Step 1: Thread theme; delete duplicate `key_hint` in `catalog/view.rs`.**
- [ ] **Step 2: Map status glyphs:** Installed → `success`, pending → `muted`, SkippedOs → `warning`, issues → `error`.
- [ ] **Step 3: Run** `cargo test catalog:: --lib` and `cargo check`.
- [ ] **Step 4:** Do not commit.

---

### Task 5: README sync + full verify

**Files:**
- Modify: `README.md` — fix stale “tagged parallel stream” if still present; document Esc quit when done, SilkCircuit note optional one line
- Optionally: `CHANGELOG.md` `[Unreleased]` Changed bullet

- [ ] **Step 1: Update README keybinding section to match help bar + runner grid.**
- [ ] **Step 2: Run** `make check && make test && make lint`.
- [ ] **Step 3:** Do not commit — report ready for `/land`.

---

## Spec coverage checklist

| Spec item | Task |
| --- | --- |
| `theme.rs` Neon tokens | 1 |
| `NO_COLOR` → mono | 1 |
| Shared key_hint / rounded borders | 2–4 |
| Help Esc quit when done; q cancel while running | 2 |
| Follow cue | 2 (keep End follow) |
| Selection accent / border_focus | 3–4 |
| Completion strip in header | 3 |
| Narrow collapse @ 68 | 3 |
| Height floor | 3 |
| Catalog shared language | 4 |
| No engine changes | all |
| README | 5 |
