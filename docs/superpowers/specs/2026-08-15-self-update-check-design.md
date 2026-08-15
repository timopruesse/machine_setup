# Self update-check notice (CLI)

Date: 2026-08-15  
Status: approved for implementation planning

## Problem

Users should learn in the terminal when a newer `machine_setup` release exists,
and see the right one-liner to upgrade based on how the binary was installed
(brew / cargo / curl installer), without slowing interactive use or breaking CI.

## Goals

- On most CLI invocations, after the command finishes, optionally print a short
  stderr notice if GitHub latest release **>** current `CARGO_PKG_VERSION`
- Print an update command from **install-method heuristics** on `current_exe()`
- Cache checks (~24h) under `temp_dir` so cold/network cost is rare
- Full disable via **env** and/or **Config** (`check_for_updates: false`)
- Fail open: network/parse errors never fail the CLI

## Non-goals (v1)

- Auto-downloading or replacing the binary
- Desktop / Notification Center alerts
- Perfect install-method detection on every exotic layout
- Checking on every noisy verb (`completions`, `schedule notify`, `schema`)

## Approach

**Cached post-command check (Approach 2):**

1. After the main command returns, call `maybe_print_update_notice(...)`.
2. If disabled (env or config) or skipped verb → return.
3. Load `~/.machine_setup/update_check.json` (under expanded `temp_dir`).
4. If last successful/attempted check is newer than 24h and we already know a
   `latest_version`, compare locally and print if newer; else fetch.
5. Fetch GitHub `releases/latest` with `ureq` and a short timeout (~2s).
6. Persist `checked_at` + `latest_version` (persist `checked_at` even on fetch
   failure to avoid hammering).
7. If remote > current, print notice + update command to **stderr**.

## Disable

| Mechanism | Behavior |
| --- | --- |
| `MACHINE_SETUP_NO_UPDATE_CHECK` ∈ `{1,true,yes}` (case-insensitive) | Skip always |
| Config `check_for_updates: false` | Skip when config is loaded |
| Omit / `true` | Enabled (default) |

Env applies even when no config is loaded. When both are set, either disable wins
(skip if env says so **or** config says false).

## Skip verbs

Do not run the check for:

- `completions`
- `schedule notify`
- `schema`

All other verbs (including `install`/`update`/`list`/`doctor`/`schedule apply`,
authoring verbs that load config) may run the check when enabled.

## Message

```text
machine_setup: new version 2.7.0 available (you have 2.6.1).
  Update: brew upgrade timopruesse/repo/machine_setup
```

Stderr only. Printed after TUI teardown when a TUI was used.

## Install method → update command

Detect from `std::env::current_exe()` (and light path probes):

| Detection | Update command |
| --- | --- |
| Path contains `Homebrew`, `Cellar`, or linuxbrew prefixes; or binary under `brew --prefix` | `brew upgrade timopruesse/repo/machine_setup` |
| Under `~/.cargo/bin` or `$CARGO_HOME/bin` | `cargo install machine_setup --force` |
| Windows best-effort (Scoop / cargo paths) | matching one-liner when confident |
| Otherwise | `curl -fsSL https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.sh \| sh` (PowerShell `irm … \| iex` on Windows) |

If ambiguous: print the curl/PS fallback **and** one short hint line naming brew and cargo alternatives.

## Version compare

- Current: `env!("CARGO_PKG_VERSION")`
- Remote: GitHub latest `tag_name`, strip leading `v`
- Notify only when remote parses as semver **greater than** current
- Equal / older / unparsable → no notice

## Config & schema

On `AppConfig`:

```rust
/// When false, skip the post-command self update-check notice (default true).
#[serde(default = "default_true")]
pub check_for_updates: bool,
```

JSON Schema: boolean property `check_for_updates` default `true`.

## Cache file

Path: `{temp_dir}/update_check.json`

```json
{
  "checked_at": "2026-08-15T12:00:00Z",
  "latest_version": "2.7.0"
}
```

TTL: 24 hours from `checked_at`.

## Module layout

| File | Role |
| --- | --- |
| `src/update_check/mod.rs` | `maybe_print_update_notice` entrypoint |
| `src/update_check/cache.rs` | load/save + TTL |
| `src/update_check/detect.rs` | path → install method → command |
| `src/update_check/fetch.rs` | GitHub latest tag |
| `src/update_check/version.rs` | semver compare helpers |
| `src/main.rs` | call after command paths |
| `src/config/types.rs` + schema | `check_for_updates` |

Prefer the name **update_check** / **self update-check** — not task `update` mode
and not the schedule “auto_update” feature.

## Edge cases

| Case | Behavior |
| --- | --- |
| Offline / timeout / 403 | Silent; still bump `checked_at` |
| TUI session | Print to stderr after UI restored |
| Command failed (nonzero tasks) | Still allow notice (informational) |
| `temp_dir` unwritable | Skip silently |
| Custom binary name / relocated brew | May fall through to curl fallback + hint |

## Testing

- Semver: `2.6.1` < `2.7.0`; `v2.7.0` strips; equal → no notify
- Detect: fake Homebrew / cargo / unknown paths → expected commands
- Cache TTL: fresh cache skips fetch (inject clock / stub)
- Env disable and `check_for_updates: false` short-circuit before fetch
- Fetch parsing of a sample GitHub JSON fixture (no live network in unit tests)

## Success criteria

1. Outdated binary prints one stderr notice with a plausible update command after a normal CLI run (cache permitting).
2. Env or config fully disables the feature.
3. Failed network never changes CLI exit behavior beyond the command’s own result.
4. No check on `completions` / `schedule notify` / `schema`.
