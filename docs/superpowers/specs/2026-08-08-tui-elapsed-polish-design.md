# TUI elapsed time and polish

Date: 2026-08-08  
Status: approved  
Repo: `timopruesse/machine_setup`

## Scope

- **Timing:** run total in the header and per-task elapsed/final duration on task rows
- **Polish:** header formatting, status glyphs, running/command hints, help-bar spacing
- **Out of scope:** plain-mode timers, engine `TaskEvent` changes, themes, mouse, auto-quit

## Approach

Keep the pure reducer. Stamp wall-clock `Instant`s when tasks start; freeze `Duration` on complete/fail/skip. Live durations are derived in widgets via `Instant::elapsed()`. Pure `format_duration` is unit-tested.

| Topic | Choice |
| --- | --- |
| Run clock | Header gauge label; freezes on `AllDone` / done |
| Per-task | Row shows live elapsed while running, frozen duration when done |
| Skip without start | No duration (unset) |
| Tick | Redraw while `!done` so the run clock advances between tasks |
| Glyphs | pending `·`, running spinner, completed `✓`, failed `✗`, skipped `–` |
| Command hint | Truncated `current_command` on running rows and log bottom title |
| Format | `<60s` → `3.4s`/`12s`; `<1h` → `1m 05s`; else `1h 02m` |
