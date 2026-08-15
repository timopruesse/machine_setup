# Auto-update schedules (OS timers + shell notices)

Date: 2026-08-15  
Status: approved for implementation planning

## Problem

Users want tooling kept fresh (e.g. bun canary, Node) on a predictable daily
cadence—typically before work starts—without manually running `update`. They
want this **per task**, with a defined clock time (and optionally cron-like
notation), bundling of tasks that share a schedule, catch-up after sleep/wake,
and a light shell hint when something was updated in the background.

This is **not** an always-on daemon and **not** an in-process “scheduler” for
the Task graph (see CONTEXT.md naming: avoid overloading “scheduler”).

## Goals

- Per-task `auto_update` in the Config document (`at: "HH:MM"` and/or daily cron)
- Hybrid model: **OS timers do the work**; **shell hook only notifies**
- Bundle tasks with the same normalized schedule into **one** OS unit
- Explicit lifecycle: `schedule apply` / `schedule remove` (install/update do not mutate timers)
- macOS (launchd user agents) + Linux (systemd **user** timers) — efficient idle path each
- Catch up after sleep/wake when the platform allows (`Persistent=true` on systemd; launchd wake behavior; document quirks)
- Reuse existing `update` mode + History `updated_at`

## Non-goals (v1)

- Windows Task Scheduler
- Non-daily cron (schema may accept a cron field; non-daily → validate error)
- Notification Center / desktop push notifications
- Forcing `exec zsh` or rewriting user shell env
- Attaching timer mutate to `install` / `update` / `uninstall`
- Polling daemons or always-on `machine_setup watch`

## Approach

**Schedule-key dispatcher:** `schedule apply` installs one OS unit per unique
daily schedule key. Each unit runs:

```text
machine_setup schedule run --key <normalized-key> --config <path-baked-at-apply>
```

`schedule run` loads the Config document **at fire time**, selects tasks that:

1. declare `auto_update` matching that key, and
2. are **installed** in History,

then runs them in **update** mode, stamps History, and appends a short notice
for the shell hook.

Adding another task with the same daily time usually needs **no new unit** (same
key). New unique times, removed keys, or config-path / binary-path changes
require re-`apply`.

## Config

```yaml
tasks:
  bun:
    auto_update:
      at: "07:30" # daily local time
    commands:
      - run:
          update: bun upgrade --canary

  node:
    auto_update:
      cron: "30 7 * * *" # equivalent daily form; mutually exclusive with `at`
    commands:
      - run:
          update: "…"
```

### Rules

| Rule | Behavior |
| --- | --- |
| Opt-in | Omit `auto_update` → task never participates in schedule units |
| Mutual exclusion | `at` and `cron` both set → validate error |
| Daily only (v1) | Non-daily cron → clear validate / apply error |
| Normalization | `"07:30"` ≡ `30 7 * * *` → same schedule **key** for bundling |
| Installed only | `schedule run` skips tasks not installed in History (no error) |
| Update semantics | Same as today’s `update` mode (including empty `run.update` behavior) |

Suggested Rust shape (illustrative):

```rust
struct AutoUpdateConfig {
    at: Option<String>,   // "HH:MM"
    cron: Option<String>, // 5-field; v1 daily only
}
```

On `TaskConfig`: `auto_update: Option<AutoUpdateConfig>`.

## CLI

| Command | Role |
| --- | --- |
| `schedule apply` | Group by key → install/refresh one unit per key; remove orphan managed units; refresh shell hook snippet (unless opted out) |
| `schedule remove` | Remove all managed units; remove hook unless `--keep-hook` |
| `schedule run --key <key>` | Timer entrypoint (also manual): update matching installed tasks; History + notice |
| `schedule status` | Managed units, tasks per key, next fire if available, last notice |
| `schedule notify` | Shell-hook target: print at most one short unseen notice, mark seen |

`install` / `update` / `uninstall` / `list` / `doctor` remain; list/doctor gain a
short `auto_update` summary (key + whether a managed unit is present).

Global `--config` applies as today; units **bake** absolute config path and
resolved `machine_setup` binary path at `apply` time so fires work outside an
interactive shell.

## OS units

### Bundling & naming

- One unit per normalized daily key (e.g. derived from `HHMM` or canonical cron)
- Label prefix: `machine_setup.schedule.<key>` (launchd label / systemd unit stem)
- Manifest under `temp_dir` (e.g. `~/.machine_setup/schedule_manifest.json`) lists
  managed unit ids for idempotent apply/remove

### macOS — launchd user agent

- Plist in `~/Library/LaunchAgents/`
- `StartCalendarInterval` for hour/minute (daily)
- ProgramArguments: absolute binary + `schedule` `run` `--key` … `--config` …
- Load/unload via current `launchctl` bootstrap/bootout (or documented equivalent)
- Catch-up: prefer behavior that runs after wake when the window was missed;
  document platform limits

### Linux — systemd user timer

- `~/.config/systemd/user/machine_setup-schedule-<key>.timer` + `.service`
- `OnCalendar=` for the daily local time
- `Persistent=true` for reboot/sleep catch-up
- `systemctl --user daemon-reload` and enable/start on apply

### Efficiency

No always-on process. launchd/systemd user timers are idle until due. Bundling
avoids N timers for N tasks that share a time. Prefer these native mechanisms
over cron.

## Shell hook & notices

### Notice store

Path under `temp_dir`, e.g. `schedule_notices.json`:

- After `schedule run`: append tasks updated (and failures), key, timestamp
- `schedule notify`: if an unseen notice exists, print one line and mark seen

Example success copy:

```text
machine_setup: updated bun, node (07:30). New shells see new binaries; version-manager shells may need a restart.
```

Example partial failure: include failed task names and point at the schedule log.

### Hook

- Installed/refreshed by `schedule apply`; removed by `schedule remove` (unless `--keep-hook`)
- v1: zsh + bash snippet (async-friendly; must not block the prompt on network)
- Calls `machine_setup schedule notify` (resolved binary path)
- Non-goals: desktop notifications, auto-`exec zsh`

### Stable-path vs version-manager

Document that fixed PATH binaries often need no shell restart (at worst `hash -r`);
nvm/fnm-style shells may need a new shell. The notice mentions this; it does not
automate it.

## State & logs

| Artifact | Purpose |
| --- | --- |
| History (`history.json`) | Existing install/update stamps; `schedule run` uses `mark_updated` |
| `schedule_manifest.json` | Managed OS unit ids for apply/remove |
| `schedule_notices.json` | Unseen/seen notices for the shell hook |
| Schedule log under `temp_dir` | Plain output from timer runs (no TTY) |

Do not invent a second “ledger”; extend History only if a field is clearly
needed later (v1 can rely on `updated_at` + notices).

## Edge cases

| Case | Behavior |
| --- | --- |
| Asleep at fire time | Catch up on wake when platform supports it |
| Task not installed | Skip silently in `schedule run` |
| Scheduled task with no update work | Same as manual `update`; warn at `apply`/`validate` when useful |
| Config path or binary moved | Re-run `schedule apply` |
| Partial task failure | History only for successes; notice includes failures |
| Laptop timezone change | Daily local time follows OS local calendar; document re-apply if units look wrong |
| Scheduled task requires sudo | Validate **Warning** (not Error). `schedule run` demotes: clears copy/symlink `sudo`, strips leading `sudo`/`sudo -n`/… from run strings, logs a warning, then continues without privileges. Interactive install/update unchanged. |

## Testing

- Parse/normalize `at` vs daily `cron`; reject non-daily / mutual exclusion
- Bundling: two tasks same time → one key
- Manifest apply/remove idempotency against a fake home (unit file content assertions)
- `schedule run` selects only installed + matching key
- Notice → notify → seen (no reprint)
- Schema / validate integration for `auto_update`

## Open implementation notes (not product forks)

- Exact key string format (`0730` vs canonical cron) — pick one and keep stable
- Whether `schedule apply` refuses non-daily at validate time only or also at apply
- launchctl API differences across macOS versions — pin to supported matrix in the plan
- Hook install location (append marker block in `~/.zshrc` vs separate sourced file) — prefer a sourced file under `temp_dir` plus a one-line source stub to ease remove

## Success criteria

1. User can declare per-task daily `auto_update`, `schedule apply`, and get one
   native timer per distinct time on macOS and Linux.
2. Timer fire updates the bundled installed tasks without an interactive TTY.
3. Next shell start can show a single notice when updates (or failures) occurred.
4. Idle cost remains negligible (no daemon); documentation covers catch-up and
   shell-restart expectations.
