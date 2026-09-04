#!/bin/bash
# Create an annotated release tag from Cargo.toml after release/check.sh passes.
# Does not bump versions or edit CHANGELOG — see docs/RELEASE.md.
#
# Usage:
#   ./release/tag.sh           # create local tag, print push commands
#   ./release/tag.sh --push    # create tag and push HEAD + tag to origin
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PUSH=0
for arg in "$@"; do
  case "$arg" in
    --push) PUSH=1 ;;
    -h|--help)
      echo "Usage: $0 [--push]"
      echo "  Runs release/check.sh, then creates annotated tag vX.Y.Z from Cargo.toml."
      echo "  --push  also runs: git push origin HEAD && git push origin vX.Y.Z"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg (try --help)" >&2
      exit 1
      ;;
  esac
done

"${ROOT}/release/check.sh"

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
TAG="v${VERSION}"

git tag -a "${TAG}" -m "${TAG}"
echo "release-tag: created annotated tag ${TAG} at $(git rev-parse --short HEAD)"

if [[ "$PUSH" -eq 1 ]]; then
  git push origin HEAD
  git push origin "${TAG}"
  echo "release-tag: pushed HEAD and ${TAG} to origin"
  echo "Watch: gh run list --branch ${TAG} --limit 5"
else
  echo "release-tag: not pushed. When ready:"
  echo "  git push origin HEAD && git push origin ${TAG}"
  echo "Or: ./release/tag.sh --push"
fi
