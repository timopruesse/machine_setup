# Auto-update schedules — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Per-task daily `auto_update` schedules applied as one native OS timer per schedule key (launchd / systemd user), with `schedule run` dispatching updates and a shell hook for notices.

**Architecture:** New `src/schedule/` module owns key normalization, manifest, notices, platform unit writers, apply/remove, and run/notify. CLI `schedule` subcommands wire through `main.rs`. Timers invoke `schedule run --key … --config …`; Config is re-loaded at fire time. No always-on daemon.

**Tech Stack:** Rust, clap, serde, chrono, existing `TaskRunner` / History, launchd plists, systemd user units.

**Spec:** `docs/superpowers/specs/2026-08-15-auto-update-schedule-design.md`

## Global Constraints

- Daily schedules only in v1 (`at: "HH:MM"` or cron `M H * * *`); non-daily cron → validate Error.
- `at` and `cron` mutually exclusive.
- Bundle by normalized key; one OS unit per key.
- Explicit `schedule apply` / `remove` only — do not mutate timers from install/update/uninstall.
- macOS launchd user agents + Linux systemd user timers (`Persistent=true`); no cron.
- Catch-up after sleep/wake when platform allows; document quirks.
- Installed-only tasks run on fire; reuse update mode + History.
- Do not call this subsystem a “scheduler” in CONTEXT/user-facing docs (prefer schedule / auto_update / OS timer).
- Parent/agent: do not commit unless the user asks (this session: user asked to implement; commit when they ask again or at wrap-up).

## File map

| File | Responsibility |
| --- | --- |
| `src/config/types.rs` | `AutoUpdateConfig` on `TaskConfig` |
| `src/config/validate.rs` | Validate auto_update fields |
| `src/config/schema.rs` + `schema/machine_setup.schema.json` | Schema for `auto_update` |
| `src/schedule/mod.rs` | Module root + public entrypoints |
| `src/schedule/key.rs` | Parse/normalize → `ScheduleKey` (`HHMM` daily) |
| `src/schedule/group.rs` | Group tasks by key from `AppConfig` |
| `src/schedule/manifest.rs` | `schedule_manifest.json` managed unit ids |
| `src/schedule/notices.rs` | `schedule_notices.json` + notify |
| `src/schedule/platform/mod.rs` | `PlatformUnits` trait |
| `src/schedule/platform/launchd.rs` | macOS plist write/load/unload |
| `src/schedule/platform/systemd.rs` | Linux user timer/service |
| `src/schedule/platform/unsupported.rs` | Clear error on other OS |
| `src/schedule/hook.rs` | Sourced hook file + stub line helpers |
| `src/schedule/apply.rs` | apply / remove |
| `src/schedule/run.rs` | `schedule run` → TaskRunner update |
| `src/schedule/status.rs` | Human status text |
| `src/cli.rs` | `Schedule` subcommands |
| `src/lib.rs` | `pub mod schedule` |
| `src/main.rs` | Dispatch schedule commands |
| `src/error.rs` | `ScheduleError` variant if needed |
| `src/tui/catalog/adapt.rs` | Show auto_update in list detail |
| `README.md` / `CHANGELOG.md` | User docs |

---

### Task 1: Config types + schedule key normalization

**Files:**
- Modify: `src/config/types.rs`
- Create: `src/schedule/key.rs`, `src/schedule/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/config/validate.rs`

**Interfaces:**
- Produces: `AutoUpdateConfig { at: Option<String>, cron: Option<String> }`
- Produces: `ScheduleKey` with `fn parse_auto_update(&AutoUpdateConfig) -> Result<ScheduleKey, String>`, `fn hour_minute(&self) -> (u32, u32)`, `fn as_str(&self) -> &str` (stable key e.g. `0730`)
- Produces: validation errors for mutual exclusion / bad format / non-daily cron

- [ ] **Step 1:** Add `auto_update: Option<AutoUpdateConfig>` to `TaskConfig` (default None via `#[serde(default)]`).
- [ ] **Step 2:** Implement `ScheduleKey` tests first:
  - `"07:30"` → key `0730`, (7, 30)
  - cron `30 7 * * *` → same key
  - `0 7 * * 1` → error non-daily
  - both at+cron → error
  - invalid `25:00` → error
- [ ] **Step 3:** Implement parse/normalize (no external cron crate — hand-parse daily forms).
- [ ] **Step 4:** Wire validate.rs to emit Errors for invalid auto_update; Warning if auto_update set but no update-mode command work (optional, if cheap via catalog).
- [ ] **Step 5:** `cargo test schedule::key` and validate unit tests pass.

### Task 2: Notices + manifest

**Files:**
- Create: `src/schedule/notices.rs`, `src/schedule/manifest.rs`

**Interfaces:**
- `NoticeStore` load/save/append/take_unseen under `temp_dir/schedule_notices.json`
- `Manifest` load/save under `temp_dir/schedule_manifest.json` with `units: Vec<ManagedUnit { key, label }>`

- [ ] **Step 1:** Unit tests for notice roundtrip and “notify marks seen”.
- [ ] **Step 2:** Implement notices + manifest.
- [ ] **Step 3:** `cargo test schedule::` green for these modules.

### Task 3: Grouping + `schedule run`

**Files:**
- Create: `src/schedule/group.rs`, `src/schedule/run.rs`

**Interfaces:**
- `fn tasks_for_key(config: &AppConfig, key: &ScheduleKey) -> Vec<String>`
- `fn group_keys(config: &AppConfig) -> Result<BTreeMap<ScheduleKey, Vec<String>>, …>`
- `async fn run_key(config, config_path, key, temp_dir, no_tui, …) -> Result<()>` — filter installed via History, `TaskRunner` in `Mode::Update`, append notice, write log under `temp_dir/schedule.log` (append)

- [ ] **Step 1:** Tests for grouping same `at` / equivalent cron into one key; skip invalid already validated.
- [ ] **Step 2:** Implement group + run (reuse runner patterns from main for force=true on update? Use `force: true` so history skip does not block updates — confirm runner only skips install-already-installed; update should always run).
- [ ] **Step 3:** Integration-style unit test with temp config + history marking one task installed.

### Task 4: Platform unit writers

**Files:**
- Create: `src/schedule/platform/{mod,launchd,systemd,unsupported}.rs`

**Interfaces:**
- Trait `PlatformUnits`: `fn apply_unit(spec: &UnitSpec) -> Result<()>`, `fn remove_unit(label: &str) -> Result<()>`, `fn list_hint(label) -> String`
- `UnitSpec { key, hour, minute, binary: PathBuf, config_path: PathBuf, label: String }`
- Label: `com.machine_setup.schedule.<key>` (launchd) / `machine_setup-schedule-<key>` (systemd)

- [ ] **Step 1:** Pure functions that render plist XML and systemd unit text — unit-test string contents (ProgramArguments, OnCalendar, Persistent=true).
- [ ] **Step 2:** Implement write + `launchctl` / `systemctl --user` calls behind `#[cfg]` / runtime OS detect; on unsupported OS return clear `Error::Other`.
- [ ] **Step 3:** Tests for renderers only in CI (do not require real launchd in Linux CI).

### Task 5: apply / remove / hook / status

**Files:**
- Create: `src/schedule/apply.rs`, `src/schedule/hook.rs`, `src/schedule/status.rs`

**Interfaces:**
- `fn apply(config, config_path, temp_dir, binary, install_hook: bool) -> Result<ApplyReport>`
- `fn remove(temp_dir, keep_hook: bool) -> Result<()>`
- Hook: write `temp_dir/schedule_hook.sh` that runs `"$binary" schedule notify`; manage marked stub in `~/.zshrc` / `~/.bashrc` between `# machine_setup schedule hook` markers (or document path and print “add: source …”).
- Prefer: write hook file always; on apply print one-line instruct if stub missing; `--install-hook` flag on apply to splice stub (safer default: file only + instructions, or install stub with markers for easy remove).

**Locked UX:** `schedule apply` refreshes hook **file**; installs/updates a **marked** one-line `source` stub in `~/.zshrc` and `~/.bashrc` when those files exist (create stub block if missing). `schedule remove` removes stub unless `--keep-hook`.

- [ ] **Step 1:** Test stub insert/remove with tempfile “home”.
- [ ] **Step 2:** apply: compute keys → for each write unit → update manifest → remove orphan labels → refresh hook.
- [ ] **Step 3:** status: print keys, tasks, manifest presence.

### Task 6: CLI + main wiring

**Files:**
- Modify: `src/cli.rs`, `src/main.rs`

```text
machine_setup schedule apply [--install-hook/--no-install-hook]
machine_setup schedule remove [--keep-hook]
machine_setup schedule run --key <key>
machine_setup schedule status
machine_setup schedule notify
```

- [ ] **Step 1:** Add clap enums.
- [ ] **Step 2:** Dispatch in main (resolve config like other commands; for `notify` only need temp_dir from config or default `~/.machine_setup`).
- [ ] **Step 3:** `cargo check` / `cargo test`.

### Task 7: Schema, catalog detail, docs

**Files:**
- Modify: `src/config/schema.rs`, regenerate `schema/machine_setup.schema.json` via `make schema`
- Modify: `src/tui/catalog/adapt.rs` — detail line for auto_update key
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1:** Schema property for auto_update object.
- [ ] **Step 2:** Catalog Meta/detail shows `auto_update: daily 07:30` when set.
- [ ] **Step 3:** README section + CHANGELOG Unreleased Added.
- [ ] **Step 4:** `make lint` and `make test` green.

---

## Spec coverage checklist

| Spec item | Task |
| --- | --- |
| Per-task at/cron | 1 |
| Daily-only + mutual exclusion | 1 |
| Key bundling | 3 |
| schedule apply/remove/run/status/notify | 5–6 |
| launchd + systemd Persistent | 4 |
| Notices + shell hook | 2, 5 |
| Installed-only run | 3 |
| list/doctor hint | 7 |
| No Windows / no install-tied timers | Global |

## Self-review

- No TBD placeholders in tasks.
- Key format locked: `HHMM` zero-padded.
- Hook default: marked stub in zshrc/bashrc when present.
- Runner: confirm update path ignores “already installed” skip (install-only) before Task 3.
