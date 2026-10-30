#!/bin/sh
set -eu

# Resolve Lowfi inside this bundle regardless of Terminal's working directory.
# Keep paths and forwarded arguments quoted so relocated bundles still work.
resource_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
lowfi="$resource_dir/../MacOS/lowfi"

if [ ! -x "$lowfi" ]; then
    echo "Lowfi executable is missing: $lowfi" >&2
    exit 1
fi

exec "$lowfi" "$@"
