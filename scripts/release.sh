#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
  echo "Usage: scripts/release.sh X.Y.Z" >&2
  exit 2
}

release_version=${1:-}
[[ $# -eq 1 ]] || usage
[[ "$release_version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(0|[1-9A-Za-z-][0-9A-Za-z-]*)(\.(0|[1-9A-Za-z-][0-9A-Za-z-]*))*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] || {
  echo "Release version must be a semantic version: $release_version" >&2
  exit 1
}

release_tag="v$release_version"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Release requires a clean working tree." >&2
  exit 1
fi
if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "Release requires the main branch." >&2
  exit 1
fi

workspace_versions=()
while IFS= read -r package_version; do
  workspace_versions+=("$package_version")
done < <(cargo metadata --locked --no-deps --format-version 1 | jq -r '.packages[].version')

[[ ${#workspace_versions[@]} -gt 0 ]] || {
  echo "No Cargo workspace packages were found." >&2
  exit 1
}
for package_version in "${workspace_versions[@]}"; do
  [[ "$package_version" == "$release_version" ]] || {
    echo "All Cargo workspace packages must use $release_version; found $package_version." >&2
    exit 1
  }
done

python_manifest_path=$(sed -n 's/^manifest-path = "\(.*\)"$/\1/p' pyproject.toml)
[[ "$python_manifest_path" == "crates/page_python/Cargo.toml" ]] || {
  echo "Python packaging must derive its version from crates/page_python/Cargo.toml." >&2
  exit 1
}
grep -Eq '^dynamic = \[.*"version"' pyproject.toml || {
  echo "Python packaging must declare a dynamic version." >&2
  exit 1
}

git rev-parse --verify --quiet "refs/tags/$release_tag" >/dev/null && {
  echo "Release tag already exists: $release_tag" >&2
  exit 1
}

git tag "$release_tag"
git push --atomic origin main "refs/tags/$release_tag"
echo "Release $release_tag pushed; GitHub Actions will release Rust and Python."
