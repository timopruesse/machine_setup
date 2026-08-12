# Shared catalog viewer (`list` + `-s`) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable catalog TUI (browse + multi-select) so `list` is browseable by default and `-s` uses the same viewer with status/detail; pretty plain `list` when TUI is unavailable.

**Architecture:** New `src/tui/catalog/` module with pure `CatalogState` / `reduce`, master–detail ratatui view, adapters from `config::status::rows`, and a colored plain printer. Run TUI stays separate. `doctor` is not implemented — only keep the item model open enough for a future adapter.

**Tech Stack:** Rust, ratatui 0.30, crossterm 0.29 (`style` for plain colors), existing `config::status`, `dialoguer` fallback for `-s`.

**Spec:** `docs/superpowers/specs/2026-08-12-catalog-viewer-list-select-design.md`

## Global Constraints

- TUI gate: `!cli.no_tui && std::io::stdout().is_terminal()`.
- Do not change engine, History semantics, or run-TUI behavior (`src/tui/state.rs` / `reduce.rs` / widgets for install).
- No doctor UI in this change.
- Plain color off when `!stdout.is_terminal()` or `NO_COLOR` is set (any value).
- Select with zero checks on Enter → abort (no tasks).
- `a` selects all **visible** (filtered) items only.
- Esc: clear filter if active (`search_mode` or non-empty query), else quit/abort.
- `q` / Ctrl+C: quit (browse) / abort (select).
- No new color crate — use `crossterm::style`.
- No ratatui golden tests; no mouse/themes/keybind config.
- Do not commit unless the user explicitly asks (leave a clean diff for `/land` or committer). Prefer a feature branch off `main` before coding.

## File map

| File | Responsibility |
| --- | --- |
| `src/tui/catalog/mod.rs` | `run_browse`, `run_select`, terminal setup, module exports |
| `src/tui/catalog/model.rs` | `CatalogItem`, `CatalogStatus`, `DetailSection`, `CatalogMode` |
| `src/tui/catalog/message.rs` | `CatalogInput`, `CatalogMessage`, `CatalogEffect` |
| `src/tui/catalog/state.rs` | `CatalogState` + helpers (`filter_active`, refresh filter, toggle) |
| `src/tui/catalog/reduce.rs` | Pure `reduce` + unit tests |
| `src/tui/catalog/adapt.rs` | `list_items` / `select_items` from `AppConfig` + `History` |
| `src/tui/catalog/plain.rs` | Pretty columnar `list` printer + tests |
| `src/tui/catalog/view.rs` | ratatui layout: list + detail + help + search |
| `src/tui/catalog/event_loop.rs` | Sync key loop for catalog (no engine channel) |
| `src/tui/mod.rs` | `pub mod catalog;` — keep `restore_terminal` reusable (`pub(crate)`) |
| `src/main.rs` | Wire `list` + `select_tasks` to catalog |
| `CHANGELOG.md` | Unreleased Added/Changed bullets |

---

### Task 1: Model, message, state + failing reducer tests

**Files:**
- Create: `src/tui/catalog/model.rs`
- Create: `src/tui/catalog/message.rs`
- Create: `src/tui/catalog/state.rs`
- Create: `src/tui/catalog/reduce.rs` (stub `reduce` + failing tests)
- Create: `src/tui/catalog/mod.rs` (module decls)
- Create: empty stubs `adapt.rs`, `plain.rs`, `view.rs`, `event_loop.rs` (filled in later tasks)
- Modify: `src/tui/mod.rs` — add `pub mod catalog;`

**Interfaces:**
- Produces:
  - `pub enum CatalogStatus { Installed, NotInstalled, SkippedOs, Neutral }`
  - `pub struct DetailSection { pub title: String, pub lines: Vec<String> }`
  - `pub struct CatalogItem { pub id: String, pub title: String, pub status: CatalogStatus, pub os_label: String, pub installed_at: String, pub updated_at: String, pub badges: Vec<String>, pub detail: Vec<DetailSection> }`
  - `pub enum CatalogMode { Browse, Select }`
  - `pub enum CatalogEffect { None, Quit, Abort, Confirm(Vec<String>) }`
  - `pub enum CatalogInput { Quit, Abort, ClearFilterOrLeave, EnterSearch, ConfirmSearch, ExitSearch, SearchChar(char), SearchBackspace, SelectNext, SelectPrev, ToggleCheck, SelectAllVisible, ConfirmSelection }`
  - `pub enum CatalogMessage { Input(CatalogInput) }`
  - `pub struct CatalogState { pub items: Vec<CatalogItem>, pub mode: CatalogMode, pub selected: usize, pub checked: BTreeSet<usize>, pub search_mode: bool, pub search_query: String, pub filtered_indices: Vec<usize> }`
  - `CatalogState::new(items: Vec<CatalogItem>, mode: CatalogMode) -> Self`
  - `CatalogState::filter_active(&self) -> bool`
  - `CatalogState::refresh_filter(&mut self)`
  - `pub fn reduce(state: CatalogState, msg: CatalogMessage) -> (CatalogState, CatalogEffect)`

- [ ] **Step 1: Add catalog module skeleton and types**

In `src/tui/mod.rs`, add `pub mod catalog;` next to the other module lines.

Create `src/tui/catalog/mod.rs`:

```rust
pub mod adapt;
pub mod event_loop;
pub mod message;
pub mod model;
pub mod plain;
pub mod reduce;
pub mod state;
pub mod view;

pub use message::{CatalogEffect, CatalogInput, CatalogMessage};
pub use model::{CatalogItem, CatalogMode, CatalogStatus, DetailSection};
pub use state::CatalogState;
```

Create empty placeholder files for modules not implemented yet (`adapt.rs`, `plain.rs`, `view.rs`, `event_loop.rs`).

Create `model.rs` with `CatalogStatus`, `DetailSection`, `CatalogItem` (include `os_label`, `installed_at`, `updated_at` from the start), and `CatalogMode`.

Create `message.rs` with `CatalogEffect`, `CatalogInput`, `CatalogMessage` (derive `Debug, Clone, PartialEq, Eq` as appropriate; `Confirm(Vec<String>)` needs `PartialEq`).

Create `state.rs`:

```rust
use std::collections::BTreeSet;

use super::model::{CatalogItem, CatalogMode};

#[derive(Debug, Clone)]
pub struct CatalogState {
    pub items: Vec<CatalogItem>,
    pub mode: CatalogMode,
    pub selected: usize,
    pub checked: BTreeSet<usize>,
    pub search_mode: bool,
    pub search_query: String,
    pub filtered_indices: Vec<usize>,
}

impl CatalogState {
    pub fn new(items: Vec<CatalogItem>, mode: CatalogMode) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            mode,
            selected: 0,
            checked: BTreeSet::new(),
            search_mode: false,
            search_query: String::new(),
            filtered_indices,
        }
    }

    pub fn filter_active(&self) -> bool {
        self.search_mode || !self.search_query.is_empty()
    }

    pub fn refresh_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        if q.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            self.filtered_indices = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.title.to_lowercase().contains(&q) || item.id.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if !self.filtered_indices.is_empty() && !self.filtered_indices.contains(&self.selected)
        {
            self.selected = self.filtered_indices[0];
        }
    }
}
```

- [ ] **Step 2: Stub `reduce` and write failing tests**

```rust
// src/tui/catalog/reduce.rs
use super::message::{CatalogEffect, CatalogInput, CatalogMessage};
use super::model::{CatalogItem, CatalogMode, CatalogStatus};
use super::state::CatalogState;

pub fn reduce(state: CatalogState, _msg: CatalogMessage) -> (CatalogState, CatalogEffect) {
    (state, CatalogEffect::None) // stub — Task 2 replaces this
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> CatalogItem {
        CatalogItem {
            id: id.into(),
            title: id.into(),
            status: CatalogStatus::NotInstalled,
            os_label: "all".into(),
            installed_at: "-".into(),
            updated_at: "-".into(),
            badges: vec![],
            detail: vec![],
        }
    }

    fn browse(ids: &[&str]) -> CatalogState {
        CatalogState::new(ids.iter().map(|s| item(s)).collect(), CatalogMode::Browse)
    }

    fn select_mode(ids: &[&str]) -> CatalogState {
        CatalogState::new(ids.iter().map(|s| item(s)).collect(), CatalogMode::Select)
    }

    #[test]
    fn quit_returns_quit_in_browse() {
        let (s, e) = reduce(browse(&["a"]), CatalogMessage::Input(CatalogInput::Quit));
        assert_eq!(e, CatalogEffect::Quit);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn abort_returns_abort_in_select() {
        let (_, e) = reduce(select_mode(&["a"]), CatalogMessage::Input(CatalogInput::Abort));
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn esc_clears_filter_without_leaving() {
        let state = browse(&["alpha", "beta"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('a')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        assert!(state.filter_active());
        let (state, e) = reduce(state, CatalogMessage::Input(CatalogInput::ClearFilterOrLeave));
        assert_eq!(e, CatalogEffect::None);
        assert!(!state.filter_active());
        assert_eq!(state.filtered_indices.len(), 2);
    }

    #[test]
    fn esc_leaves_when_no_filter() {
        let (_, e) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ClearFilterOrLeave),
        );
        assert_eq!(e, CatalogEffect::Quit);
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::ClearFilterOrLeave),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn navigate_stays_within_filtered_set() {
        let state = browse(&["alpha", "beta", "gamma"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('a')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectNext));
        assert!(state.filtered_indices.contains(&state.selected));
    }

    #[test]
    fn toggle_check_only_in_select_mode() {
        let (state, _) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ToggleCheck),
        );
        assert!(state.checked.is_empty());

        let (state, _) = reduce(
            select_mode(&["a", "b"]),
            CatalogMessage::Input(CatalogInput::ToggleCheck),
        );
        assert!(state.checked.contains(&0));
    }

    #[test]
    fn select_all_visible_checks_filtered_only() {
        let state = select_mode(&["alpha", "beta", "gamma"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('a')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectAllVisible));
        for &i in &state.filtered_indices {
            assert!(state.checked.contains(&i));
        }
        let beta = state.items.iter().position(|it| it.id == "beta").unwrap();
        assert!(!state.checked.contains(&beta));
    }

    #[test]
    fn confirm_with_checks_returns_ids() {
        let state = select_mode(&["a", "b"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ToggleCheck));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectNext));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ToggleCheck));
        let (_, e) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSelection));
        match e {
            CatalogEffect::Confirm(ids) => assert_eq!(ids, vec!["a".to_string(), "b".to_string()]),
            other => panic!("expected Confirm, got {other:?}"),
        }
    }

    #[test]
    fn confirm_with_zero_checks_aborts() {
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::ConfirmSelection),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn confirm_ignored_in_browse() {
        let (_, e) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ConfirmSelection),
        );
        assert_eq!(e, CatalogEffect::None);
    }
}
```

- [ ] **Step 3: Run tests — expect failures**

Run: `cargo test --lib catalog::reduce:: -- --nocapture`  
Expected: compile OK; asserts fail because stub always returns `CatalogEffect::None` and never mutates checks/filter.

---

### Task 2: Implement `reduce`

**Files:**
- Modify: `src/tui/catalog/reduce.rs`

**Interfaces:**
- Consumes: Task 1 types
- Produces: working `reduce`

- [ ] **Step 1: Replace stub with full input handling**

```rust
use super::message::{CatalogEffect, CatalogInput, CatalogMessage};
use super::model::CatalogMode;
use super::state::CatalogState;

pub fn reduce(mut state: CatalogState, msg: CatalogMessage) -> (CatalogState, CatalogEffect) {
    let CatalogMessage::Input(input) = msg;
    match input {
        CatalogInput::Quit => {
            let effect = match state.mode {
                CatalogMode::Browse => CatalogEffect::Quit,
                CatalogMode::Select => CatalogEffect::Abort,
            };
            (state, effect)
        }
        CatalogInput::Abort => (state, CatalogEffect::Abort),
        CatalogInput::ClearFilterOrLeave => {
            if state.filter_active() {
                state.search_mode = false;
                state.search_query.clear();
                state.refresh_filter();
                (state, CatalogEffect::None)
            } else {
                let effect = match state.mode {
                    CatalogMode::Browse => CatalogEffect::Quit,
                    CatalogMode::Select => CatalogEffect::Abort,
                };
                (state, effect)
            }
        }
        CatalogInput::EnterSearch => {
            state.search_mode = true;
            (state, CatalogEffect::None)
        }
        CatalogInput::ExitSearch => {
            state.search_mode = false;
            state.search_query.clear();
            state.refresh_filter();
            (state, CatalogEffect::None)
        }
        CatalogInput::ConfirmSearch => {
            state.search_mode = false;
            state.refresh_filter();
            (state, CatalogEffect::None)
        }
        CatalogInput::SearchChar(c) => {
            if state.search_mode {
                state.search_query.push(c);
                state.refresh_filter();
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SearchBackspace => {
            if state.search_mode {
                state.search_query.pop();
                state.refresh_filter();
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SelectNext | CatalogInput::SelectPrev => {
            if state.filtered_indices.is_empty() {
                return (state, CatalogEffect::None);
            }
            let go_next = matches!(input, CatalogInput::SelectNext);
            let pos = state
                .filtered_indices
                .iter()
                .position(|&i| i == state.selected)
                .unwrap_or(0);
            let next = if go_next {
                (pos + 1) % state.filtered_indices.len()
            } else if pos == 0 {
                state.filtered_indices.len() - 1
            } else {
                pos - 1
            };
            state.selected = state.filtered_indices[next];
            (state, CatalogEffect::None)
        }
        CatalogInput::ToggleCheck => {
            if matches!(state.mode, CatalogMode::Select)
                && state.filtered_indices.contains(&state.selected)
            {
                if !state.checked.remove(&state.selected) {
                    state.checked.insert(state.selected);
                }
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SelectAllVisible => {
            if matches!(state.mode, CatalogMode::Select) {
                for &i in &state.filtered_indices {
                    state.checked.insert(i);
                }
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::ConfirmSelection => {
            if !matches!(state.mode, CatalogMode::Select) {
                return (state, CatalogEffect::None);
            }
            if state.checked.is_empty() {
                return (state, CatalogEffect::Abort);
            }
            let ids: Vec<String> = state
                .checked
                .iter()
                .filter_map(|&i| state.items.get(i).map(|it| it.id.clone()))
                .collect();
            (state, CatalogEffect::Confirm(ids))
        }
    }
}
```

Keep the `#[cfg(test)]` module from Task 1.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib catalog::reduce::`  
Expected: all PASS.

---

### Task 3: Adapters (`list_items` / `select_items`)

**Files:**
- Modify: `src/tui/catalog/adapt.rs`
- Uses: `crate::config::status`, `crate::config::types::AppConfig`, `crate::config::history::History`

**Interfaces:**
- Produces:
  - `pub fn list_items(config: &AppConfig, history: &History) -> Vec<CatalogItem>`
  - `pub fn select_items(config: &AppConfig, history: &History) -> Vec<CatalogItem>` (delegate to `list_items`)

- [ ] **Step 1: Write failing adapter test**

Copy the `empty_task` / `AppConfig` setup from `src/config/status.rs` tests. Assert installed vs not-installed status and that a `History` detail section exists.

- [ ] **Step 2: Run test — expect fail**

Run: `cargo test --lib catalog::adapt::`

- [ ] **Step 3: Implement adapters**

```rust
use crate::config::history::History;
use crate::config::status::{self, format_ts, os_label};
use crate::config::types::AppConfig;

use super::model::{CatalogItem, CatalogStatus, DetailSection};

pub fn list_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    status::rows(config, history)
        .into_iter()
        .map(row_to_item)
        .collect()
}

pub fn select_items(config: &AppConfig, history: &History) -> Vec<CatalogItem> {
    list_items(config, history)
}

fn row_to_item(row: status::TaskStatusRow<'_>) -> CatalogItem {
    let status = if !row.os_applies {
        CatalogStatus::SkippedOs
    } else if row.installed {
        CatalogStatus::Installed
    } else {
        CatalogStatus::NotInstalled
    };

    let mut badges = Vec::new();
    if row.task.parallel {
        badges.push("parallel".into());
    }
    if !row.os_applies {
        badges.push("os skip".into());
    }

    let (installed_at, updated_at) = match row.history {
        Some(h) => (format_ts(h.installed_at), format_ts(h.updated_at)),
        None => ("-".into(), "-".into()),
    };

    let os = os_label(&row.task.os);

    let mut detail = vec![
        DetailSection {
            title: "Meta".into(),
            lines: vec![
                format!("OS: {os}"),
                format!("Installed: {}", if row.installed { "yes" } else { "no" }),
            ],
        },
        DetailSection {
            title: "History".into(),
            lines: vec![
                format!("installed_at: {installed_at}"),
                format!("updated_at: {updated_at}"),
            ],
        },
    ];

    let cmd_lines: Vec<String> = row.task.commands.iter().map(|c| format!("- {c}")).collect();
    detail.push(DetailSection {
        title: "Commands".into(),
        lines: if cmd_lines.is_empty() {
            vec!["(none)".into()]
        } else {
            cmd_lines
        },
    });

    CatalogItem {
        id: row.name.to_string(),
        title: row.name.to_string(),
        status,
        os_label: os,
        installed_at,
        updated_at,
        badges,
        detail,
    }
}
```

- [ ] **Step 4: Run adapter tests**

Run: `cargo test --lib catalog::adapt::`  
Expected: PASS.

---

### Task 4: Pretty plain printer + temporary `list` wiring

**Files:**
- Modify: `src/tui/catalog/plain.rs`
- Modify: `src/main.rs` — replace `print_task_list` to call `plain::print_list` (TUI branch added in Task 6)

**Interfaces:**
- Produces:
  - `pub fn color_enabled() -> bool`
  - `pub fn render_list(items: &[CatalogItem], color: bool) -> String`
  - `pub fn print_list(items: &[CatalogItem])`

- [ ] **Step 1: Failing tests for `render_list`**

Assert glyph/name/command presence with `color: false`. Assert empty list message contains `No tasks` or `0`.

- [ ] **Step 2: Implement renderer**

- Header: `Tasks  (N total · M installed)`
- Row: glyph + title + os_label + installed_at + updated_at + badges (space-separated)
- Glyphs: Installed `✓`, NotInstalled `·`, SkippedOs `–`, Neutral `·`
- Under each row: indented `detail` Commands lines (or all detail lines muted)
- When `color`: use `crossterm::style::Stylize` (`green`/`dark_grey`/`yellow`); when `!color`, raw text only
- `color_enabled`: `std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()`

- [ ] **Step 3: Wire `print_task_list` in `main.rs`**

```rust
fn print_task_list(config: &config::types::AppConfig) {
    use machine_setup::tui::catalog::{adapt, plain};
    use machine_setup::utils::path::expand_path;

    let temp_dir = expand_path(&config.temp_dir, None);
    let history = config::history::History::load(&temp_dir).unwrap_or_default();
    let items = adapt::list_items(config, &history);
    plain::print_list(&items);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib catalog::plain::` and `cargo check`  
Expected: PASS / OK.

---

### Task 5: View + event loop + runners

**Files:**
- Modify: `src/tui/catalog/view.rs`
- Modify: `src/tui/catalog/event_loop.rs`
- Modify: `src/tui/catalog/mod.rs` — add `run_browse` / `run_select`

**Interfaces:**
- Produces:
  - `view::render(f: &mut Frame, state: &CatalogState)`
  - `event_loop::LoopOutcome { Quit, Abort, Confirm(Vec<String>) }`
  - `event_loop::run(terminal, state) -> anyhow::Result<LoopOutcome>`
  - `pub fn run_browse(items: Vec<CatalogItem>) -> anyhow::Result<()>`
  - `pub fn run_select(items: Vec<CatalogItem>) -> anyhow::Result<Option<Vec<String>>>`

- [ ] **Step 1: Implement master–detail `view.rs`**

Vertical: main (`Min`) + help (`Length(1)`).  
Horizontal main: list `Percentage(40)` + detail `Percentage(60)`.  
List: `>` selected; Select mode `[x]`/`[ ]`; status glyph; title; badges.  
Detail: sections from selected item.  
Help: mode-specific hints.  
Search: show `/query` when `filter_active()` (above help or under list).

- [ ] **Step 2: Implement sync `event_loop.rs`**

Map keys (see spec): `q`, Esc, `/`, j/k, Space, `a`, Enter, Ctrl+C.  
Ignore key release; allow Repeat for j/k.  
Call `reduce` until non-`None` effect.

- [ ] **Step 3: Implement `run_browse` / `run_select` in `mod.rs`**

Same terminal bootstrap as `tui::run` (panic hook → `restore_terminal`, raw mode, alternate screen).  
Empty items: browse prints `No tasks defined.` and returns Ok; select returns `Ok(None)`.

- [ ] **Step 4: `cargo check`**

Expected: OK.

---

### Task 6: Wire `list` TUI vs plain

**Files:**
- Modify: `src/main.rs` list branch

- [ ] **Step 1: Branch on TUI gate**

```rust
if cli.command == Command::List {
    use std::io::IsTerminal;
    use machine_setup::tui::catalog::{adapt, plain, run_browse};
    use machine_setup::utils::path::expand_path;

    let temp_dir = expand_path(&app_config.temp_dir, None);
    let history = config::history::History::load(&temp_dir).unwrap_or_default();
    let items = adapt::list_items(&app_config, &history);
    let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
    if use_tui {
        run_browse(items)?;
    } else {
        plain::print_list(&items);
    }
    return Ok(());
}
```

Delete unused `print_task_list` if fully inlined.

- [ ] **Step 2: Manual smoke**

`cargo run -- list -c ./example_config.yaml` → TUI.  
`cargo run -- list -c ./example_config.yaml --no-tui` → plain.  
`NO_COLOR=1 cargo run -- list -c ./example_config.yaml --no-tui` → no ANSI.

- [ ] **Step 3: `cargo test --lib` and clippy `-D warnings`**

---

### Task 7: Wire `-s` to catalog select

**Files:**
- Modify: `src/main.rs` — `select_tasks` + call site

- [ ] **Step 1: Pass TUI gate into `select_tasks`**

```rust
} else if cli.select {
    let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
    select_tasks(&app_config, use_tui)?
}
```

- [ ] **Step 2: Implement dual-path `select_tasks`**

If `use_tui`: `adapt::select_items` + `run_select` → `Ok(ids)` or `Ok(vec![])` on abort.  
Else if stdin TTY: existing `dialoguer::MultiSelect`.  
Else: `anyhow::bail!("cannot select tasks interactively (no TTY); omit -s or pass -t")`.

- [ ] **Step 3: Manual smoke**

`cargo run -- install -s -c ./example_config.yaml` → catalog select.  
`cargo run -- install -s --no-tui -c ./example_config.yaml` → dialoguer.

- [ ] **Step 4: Full verify**

Run: `make check && make test && make lint`  
Expected: all green.

---

### Task 8: Changelog

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Under `[Unreleased]`**

```markdown
### Added
- Shared catalog TUI for browsing tasks (`list`) with master–detail view and `/` filter
- `-s` / `--select` uses the catalog multi-select TUI when available (status + detail)

### Changed
- `list` uses the TUI by default on a TTY; `--no-tui` / non-TTY falls back to a colored columnar plain view (`NO_COLOR` respected)
```

- [ ] **Step 2: Re-run `make test` / `make lint`**

---

## Spec coverage checklist

| Spec requirement | Task |
| --- | --- |
| Shared `src/tui/catalog/` framework | 1–5 |
| Browse master–detail + `/` | 5–6 |
| Select Space / `a` / Enter / abort | 2, 5, 7 |
| Pretty plain + `NO_COLOR` / non-TTY | 4 |
| TUI gate + `--no-tui` | 6–7 |
| dialoguer fallback for `-s` | 7 |
| Adapters from `status::rows` | 3 |
| Doctor deferred; model reusable | 3 (`CatalogItem` + Browse) |
| Run TUI untouched | Global constraint |
| Reducer / adapter / plain tests | 1–4 |

## Plan self-review

- Spec coverage: all locked decisions mapped to tasks; doctor explicitly deferred.
- No TBD placeholders; `CatalogItem` column fields defined in Task 1 to avoid Task 4 churn.
- `Quit` in select mode → `Abort` so `q` never confirms.
- Uninstall dialoguer prompts remain out of scope.
