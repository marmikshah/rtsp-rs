#!/usr/bin/env bash
# Sync version to all Cargo.toml and pyproject.toml.
# Usage: ./scripts/set-version.sh [VERSION]
#   If VERSION is given, use it and write it to VERSION file.
#   Otherwise read from VERSION file in repo root.
# The release workflow still sets version from the git tag at publish time,
# so you can also just tag vX.Y.Z without running this; this script keeps
# the repo's files in sync when you want them to match the release version.

set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ $# -ge 1 ]]; then
  VERSION="$1"
  echo "$VERSION" > VERSION
else
  if [[ ! -f VERSION ]]; then
    echo "No VERSION file and no version argument. Usage: $0 [X.Y.Z]" >&2
    exit 1
  fi
  VERSION=$(cat VERSION | tr -d '[:space:]')
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Invalid version: $VERSION (expected X.Y.Z)" >&2
  exit 1
fi

echo "Setting version to $VERSION"

sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" crates/core/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" crates/python/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" crates/gst/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" crates/python/pyproject.toml

# Clean up backup files (sed -i.bak is portable; on macOS sed -i without backup is different)
rm -f crates/core/Cargo.toml.bak crates/python/Cargo.toml.bak crates/gst/Cargo.toml.bak crates/python/pyproject.toml.bak

echo "Updated crates/core/Cargo.toml, crates/python/Cargo.toml, crates/gst/Cargo.toml, crates/python/pyproject.toml and VERSION."
