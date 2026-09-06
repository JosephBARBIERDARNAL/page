#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

release_tag=${1:?Usage: check-release-version.sh vX.Y.Z}
for package in page_cli page_validation page_python; do
  package_id=$(cargo pkgid --locked --package "$package")
  package_version=${package_id##*#}
  package_version=${package_version##*@}
  if [[ "$release_tag" != "v$package_version" ]]; then
    echo "Release tag $release_tag does not match $package version $package_version." >&2
    exit 1
  fi
done
