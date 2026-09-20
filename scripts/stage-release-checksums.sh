#!/bin/sh
set -eu

artifacts=${1:?artifact directory is required}
release=${2:?release directory is required}

mkdir -p "$release"
find "$artifacts" -type f -name 'mcpctl-*' -exec cp {} "$release"/ \;

(
  cd "$release"
  sha256sum mcpctl-* > SHA256SUMS
  sha256sum -c SHA256SUMS
)
