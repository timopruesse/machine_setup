# Releasing machine_setup

How to cut a GitHub release. Follow this instead of reverse-engineering git history.

## Preconditions

- On `main`, up to date with `origin/main`
- Changes for the release are already merged and green (`make lint`, `make test`)
- You know the next semver (see below)

## Semver

| Bump | When |
|------|------|
| **patch** (`2.9.0` → `2.9.1`) | Fixes, dependency bumps, internal refactors with no Config document / CLI surface change |
| **minor** (`2.9.0` → `2.10.0`) | New Config fields, CLI verbs, or user-visible behavior |
| **major** | Breaking Config / CLI changes |

Current version lives in `Cargo.toml` (`package.version`). Latest tag: `git tag -l 'v*' | sort -V | tail -1`.

## Helpers

| Command | Role |
|---------|------|
| `make release-check` / `bash release/check.sh` | Read-only: version ↔ CHANGELOG ↔ lockfile, clean tree, on `main`, tag not present locally or on origin |
| `make release-tag` / `bash release/tag.sh` | Runs check, creates annotated `vX.Y.Z` from `Cargo.toml`, prints push commands |
| `bash release/tag.sh --push` | Same, then `git push origin HEAD` and `git push origin vX.Y.Z` |

Neither script bumps the version or edits the changelog — that stays manual.

## Steps

### 1. Changelog

Edit `CHANGELOG.md` (Keep a Changelog):

1. Keep a top `## [Unreleased]` section (can be empty).
2. Insert `## [X.Y.Z]` below it with `### Added` / `### Changed` / `### Fixed` / `### Performance` as needed.
3. The GitHub release body is extracted by CI from the `## [X.Y.Z]` block — write notes users care about, not a file list.

### 2. Version bump

```bash
# Cargo.toml package.version → X.Y.Z
# Then sync the lockfile entry for this package:
cargo update -p machine_setup
```

### 3. Commit

Stage only release metadata:

- `CHANGELOG.md`
- `Cargo.toml`
- `Cargo.lock`

```bash
git commit -m "$(cat <<'EOF'
chore: release vX.Y.Z

EOF
)"
```

Match existing history (`chore: release v2.8.1`, `chore: release v2.9.0`).

### 4. Check, tag, push

```bash
make release-check          # or: bash release/check.sh
bash release/tag.sh --push  # check + annotated tag + push HEAD and tag
```

Or tag locally first, then push yourself:

```bash
make release-tag
git push origin HEAD && git push origin vX.Y.Z
```

Do **not** force-push tags or main.

### 5. Wait for CI

Pushing a `v*` tag triggers:

| Workflow | Trigger | Role |
|----------|---------|------|
| **Builds** (`.github/workflows/build.yml`) | push tag `v*` | Cross-compile, package artifacts, create GitHub Release (notes from CHANGELOG) |
| **Tests** (`.github/workflows/test.yml`) | push tag `v*` | Test suite on the tag |
| **Cargo Release** (`.github/workflows/publish_cargo.yml`) | `release: published` | `cargo publish` |
| **update_tap** (`.github/workflows/update_tap.yml`) | `release: published` | Homebrew tap formula bump |

Watch:

```bash
gh run list --branch vX.Y.Z --limit 5
gh release view vX.Y.Z   # appears after Builds finishes
```

Release URL pattern: `https://github.com/timopruesse/machine_setup/releases/tag/vX.Y.Z`

## What agents should not do

- Do not invent a one-off release path — use this doc and `release/*.sh`.
- Do not create a release with `gh release create` by hand unless Builds failed and you are repairing; the normal path is tag → Builds → ncipollo/release-action.
- Do not bump version without CHANGELOG notes for that version (CI falls back to auto-generated notes only if the section is missing).
- Do not use a script that invents the version or writes the changelog.

## Checklist (copy/paste)

```text
[ ] Semver decided (patch / minor / major)
[ ] CHANGELOG.md: ## [Unreleased] kept; ## [X.Y.Z] filled
[ ] Cargo.toml version = X.Y.Z
[ ] cargo update -p machine_setup
[ ] Commit: chore: release vX.Y.Z (CHANGELOG + Cargo.toml + Cargo.lock only)
[ ] make release-check
[ ] bash release/tag.sh --push
[ ] gh run list --branch vX.Y.Z  (Builds green)
[ ] gh release view vX.Y.Z       (artifacts present)
```
