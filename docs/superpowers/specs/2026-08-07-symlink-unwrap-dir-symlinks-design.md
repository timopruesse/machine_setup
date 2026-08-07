# Symlink tree walks: unwrap intermediate directory symlinks

Date: 2026-08-07  
Status: approved  
Repo: `timopruesse/machine_setup`  
Approach: **A** — always unwrap intermediate dir symlinks during directory `symlink` walks

## Context

Directory-mode `symlink` (e.g. `src: ./home`, `target: ~`) walks the source
tree and:

- creates **real directories** at corresponding dest paths
- creates **file symlinks** for each non-directory entry

`.dotfiles` previously worked around a failure mode with a YAML `run` that
unwraps top-level `~/agents|protocols|commands|skills` if those paths are
directory symlinks into the repo. That is incomplete: a **nested** leftover
such as `~/skills/route-agents → <repo>/home/skills/route-agents` still causes
`force` (and even non-force creates of new leaves) to write through into the
source tree, replacing e.g. `SKILL.md` with a broken self-symlink.

Root cause in `symlink.rs`: `mkdir` treats an existing directory symlink as
“already a directory” via `path.is_dir()` (which follows links), so the link
is left in place and later leaf operations resolve into the source.

## Goals

- Make directory `symlink` walks safe by construction: intermediate dest paths
  are always real directories.
- Always unwrap (not only when `force: true`), because new leaves under a
  leftover dir symlink can still self-link without force.
- Unwrap by removing **only the symlink inode**, never by deleting the tree it
  points at.
- Keep existing `force` semantics for **leaf** file/symlink replacement.
- Cover the behavior with unit/integration tests so the self-link regression
  cannot return silently.
- After a released fix, let `.dotfiles` drop its YAML unwrap shell block.

## Non-goals

- New YAML knobs (`unwrap_dir_symlinks`, etc.).
- Changing copy-command behavior.
- Changing single-file `symlink` mode beyond a self-link guard if cheap.
- Automatically repairing already-corrupted source files in consumer repos
  (callers restore via git).
- Windows-specific alternative semantics beyond “same unwrap rules using the
  existing symlink APIs.”

## Decisions (locked)

| Topic | Choice |
| --- | --- |
| Approach | A — fix tree-walk `mkdir` / ensure-dir |
| When to unwrap | Always on directory `symlink` install/update walks |
| What to unwrap | Any dest path that is a symlink and is being ensured as a directory (intermediates from `target` through each walked dir) |
| How to unwrap | `symlink_metadata` → if symlink, log + unlink the link + `create_dir` / `create_dir_all` as appropriate |
| `force` | Unchanged for leaves; unwrap of intermediates is independent of `force` |
| Self-link guard | Before creating a leaf symlink, if the resolved dest path equals the resolved source path (or would write into the resolved source file), error — do not create |
| Config surface | None |
| TUI | Log lines only (existing `ctx.log`); no new widgets |
| Dotfiles follow-up | Remove unwrap `run` from `machine_setup.yaml` once this ships; restore any damaged sources locally |

## Behavior

### Directory mode (`src` is a directory)

For each walked directory destination `dest`:

1. If `dest` does not exist → create a real directory (`mkdir` / `create_dir_all` as today, including sudo path).
2. If `dest` exists as a **real directory** → leave it.
3. If `dest` exists as a **symlink** (to a file or directory):
   - Log: unwrapping directory symlink at `dest`.
   - Remove the symlink only (`remove_file` / `sudo_remove` — **not**
     `remove_dir_all` on the followed target).
   - Create a real directory at `dest`.
4. If `dest` exists as a regular file (not a symlink): return a path error —
   do not delete it to make a directory. (Tree walks should not invent
   force-clobber for “file where a directory is required.”)

Ancestor components between `target` and `dest` must go through the same
ensure-real-dir logic so a top-level `~/skills → repo/home/skills` is unwrapped
before children are linked. Prefer ensuring each path prefix once (e.g. walk
from `target` toward `dest`, or ensure on every directory entry including
`target` before the walk).

### File leaves

Unchanged:

- exists + `force` → remove leaf and recreate symlink
- exists + !`force` → skip
- missing → create symlink

Plus self-link guard: before calling `symlink(src, dest)`, if `dest` already
exists and `canonicalize(src) == canonicalize(dest)`, return a path error.
After intermediates are unwrapped, the nested-dir-symlink bug should not
reach this path; the guard is belt-and-suspenders for odd force/replace
races. The integration test (source file remains a regular file with original
content; dest is a symlink to it) is the required outcome.

### Single-file mode

No intermediate-dir unwrap walk. Apply the same self-link guard when creating
the one symlink.

### Uninstall

No change required for this bug. Optional later: do not follow dir symlinks
when removing; out of scope unless tests show damage.

## Implementation sketch

Primary file: `src/engine/commands/symlink.rs`.

- Replace or wrap `mkdir` used by directory walks with `ensure_real_dir(path, use_sudo, ctx)` that implements the four cases above.
- Call it for every directory entry in `walk_relative` (and ensure `target` itself is a real dir before the walk).
- In `create_symlink`, after force-removal / before `symlink`, compare
  canonical (or `symlink_metadata`-aware) paths and return `Error::PathError`
  on self-link.
- Sudo path: use existing `sudo_remove` + `sudo_mkdir`; never `sudo_remove_dir`
  on a symlink that points at the source tree.
- Document in README under `symlink`: intermediate destinations are always
  real directories; leftover directory symlinks are replaced by empty real
  dirs (link removed, target tree untouched).
- CHANGELOG `[Unreleased]` Fixed entry.

## Tests

Add an integration test that reproduces the `.dotfiles` failure:

1. Temp `src/skills/route-agents/SKILL.md` with known content.
2. Temp `target/skills` as a **real** dir.
3. Create `target/skills/route-agents` as a **directory symlink** into
   `src/skills/route-agents`.
4. Run directory `symlink` with `force: true` (and a second case without force
   adding a new leaf).
5. Assert:
   - `target/skills/route-agents` is a real directory (not a symlink).
   - `target/skills/route-agents/SKILL.md` is a symlink to the source file.
   - Source `SKILL.md` is still a regular file with original content (not a
     self-symlink).

Unit-test `ensure_real_dir` cases if extracted to a testable helper.

## Dotfiles migration (separate PR / after release)

1. Restore any damaged sources (`git restore home/skills/route-agents/SKILL.md`).
2. Replace leftover `~/skills/route-agents` dir symlink with a real directory
   (or re-run setup after upgrading `machine_setup`).
3. Delete the unwrap `run` block in `.dotfiles` `machine_setup.yaml` (the
   comment about self-links can point at this changelog/behavior instead).

## Success criteria

- Re-running the integration fixture cannot produce a self-symlink in `src`.
- Nested and top-level leftover dir symlinks under `target` are unwrapped
  without deleting the source tree.
- No new config keys; README + CHANGELOG describe the behavior.
- `.dotfiles` can drop its YAML unwrap workaround after upgrading.

## Spec self-review (2026-08-07)

- No TBD/TODO placeholders.
- Unwrap-always vs force-for-leaves is consistent across Goals / Decisions /
  Behavior.
- Scope is one `symlink.rs` behavior change + tests + docs; dotfiles YAML
  cleanup is an explicit follow-up after release.
- Regular-file-where-dir-required is an explicit error (not force-clobber).
