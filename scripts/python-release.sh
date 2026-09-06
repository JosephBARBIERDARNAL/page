#!/bin/bash
# Both packages share one version and release tag.
exec bash "$(dirname "$0")/release.sh" "$@"
