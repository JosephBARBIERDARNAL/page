#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

release_version=${1:?Usage: release.sh X.Y.Z}
release_tag="v$release_version"
bash scripts/check-release-version.sh "$release_tag"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Commit the version bump and other changes before releasing." >&2
  exit 1
fi
if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "Releases must be made from main." >&2
  exit 1
fi

git tag "$release_tag"
git push --atomic origin main "refs/tags/$release_tag"
echo "Release $release_tag pushed; GitHub Actions will release Rust and Python."
