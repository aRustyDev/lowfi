#!/bin/sh
set -eu

resource_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
lowfi="$resource_dir/../MacOS/lowfi"

if [ ! -x "$lowfi" ]; then
    echo "Lowfi executable is missing: $lowfi" >&2
    exit 1
fi

exec "$lowfi" "$@"
