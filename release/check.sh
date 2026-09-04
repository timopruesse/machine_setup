#!/bin/bash
# Read-only release readiness checks. See docs/RELEASE.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "release-check: $*" >&2
  echo "See docs/RELEASE.md" >&2
  exit 1
}

ok() {
  echo "  ok: $*"
}

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
[[ -n "$VERSION" ]] || fail "could not read package.version from Cargo.toml"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "version '$VERSION' is not X.Y.Z"

TAG="v${VERSION}"
echo "release-check: version ${VERSION} (tag ${TAG})"

LOCK_VERSION="$(
  awk '
    $0 == "name = \"machine_setup\"" { in_pkg = 1; next }
    in_pkg && /^version = "/ {
      sub(/^version = "/, "")
      sub(/"$/, "")
      print
      exit
    }
    in_pkg && /^\[\[package\]\]/ { exit }
  ' Cargo.lock
)"
[[ -n "$LOCK_VERSION" ]] || fail "could not find machine_setup version in Cargo.lock"
[[ "$LOCK_VERSION" == "$VERSION" ]] || fail "Cargo.lock has machine_setup ${LOCK_VERSION}, Cargo.toml has ${VERSION} (run: cargo update -p machine_setup)"
ok "Cargo.lock matches Cargo.toml"

grep -q '^## \[Unreleased\]$' CHANGELOG.md || fail "CHANGELOG.md missing '## [Unreleased]' heading"
ok "CHANGELOG has ## [Unreleased]"

grep -q "^## \[${VERSION}\]$" CHANGELOG.md || fail "CHANGELOG.md missing '## [${VERSION}]' heading"
ok "CHANGELOG has ## [${VERSION}]"

if [[ -n "$(git status --porcelain)" ]]; then
  fail "working tree is dirty; commit release metadata first (CHANGELOG.md, Cargo.toml, Cargo.lock)"
fi
ok "working tree clean"

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[[ "$BRANCH" == "main" ]] || fail "expected branch main, on '${BRANCH}'"
ok "on main"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  fail "local tag ${TAG} already exists"
fi
ok "local tag ${TAG} does not exist"

if [[ "${RELEASE_CHECK_SKIP_REMOTE:-}" == "1" ]]; then
  ok "skipped remote tag check (RELEASE_CHECK_SKIP_REMOTE=1)"
else
  # --exit-code: 0 = ref exists, 2 = missing. Avoid hanging on auth prompts.
  set +e
  GIT_TERMINAL_PROMPT=0 git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1
  remote_rc=$?
  set -e
  if [[ "$remote_rc" -eq 0 ]]; then
    fail "remote tag ${TAG} already exists on origin"
  elif [[ "$remote_rc" -ne 2 ]]; then
    fail "could not query origin for ${TAG} (git ls-remote exit ${remote_rc}); retry or RELEASE_CHECK_SKIP_REMOTE=1"
  fi
  ok "remote tag ${TAG} does not exist on origin"
fi

echo "release-check: all checks passed"
