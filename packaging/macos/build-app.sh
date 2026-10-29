#!/bin/sh
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
    echo "Lowfi.app packaging requires macOS." >&2
    exit 1
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/../.." && pwd)
host_target=$(rustc -vV | sed -n 's/^host: //p')

if [ -z "$host_target" ]; then
    echo "Unable to determine the native Rust target." >&2
    exit 1
fi

metadata=$(cd "$project_dir" && cargo metadata --locked --no-deps --format-version 1)
package_version=$(printf '%s' "$metadata" | plutil -extract packages.0.version raw -o - -)
target_dir=$(printf '%s' "$metadata" | plutil -extract target_directory raw -o - -)
release_dir="$target_dir/$host_target/release"
bundle_dir="$release_dir/bundle/Lowfi.app"

(cd "$project_dir" && cargo build --locked --release --target "$host_target" --features macos-app --bin lowfi --bin lowfi-app-launcher)
stage_dir=$(mktemp -d "$release_dir/.lowfi-app.XXXXXX")

cleanup() {
    rm -rf "$stage_dir"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$stage_dir/Lowfi.app/Contents/MacOS" "$stage_dir/Lowfi.app/Contents/Resources"
cp "$release_dir/lowfi" "$stage_dir/Lowfi.app/Contents/MacOS/lowfi"
cp "$release_dir/lowfi-app-launcher" "$stage_dir/Lowfi.app/Contents/MacOS/lowfi-app-launcher"
cp "$script_dir/Lowfi.command" "$stage_dir/Lowfi.app/Contents/Resources/Lowfi.command"
cp "$script_dir/Lowfi.icns" "$stage_dir/Lowfi.app/Contents/Resources/Lowfi.icns"
sed "s/@VERSION@/$package_version/g" "$script_dir/Info.plist.in" > "$stage_dir/Lowfi.app/Contents/Info.plist"
chmod 755 "$stage_dir/Lowfi.app/Contents/MacOS/lowfi" "$stage_dir/Lowfi.app/Contents/MacOS/lowfi-app-launcher" "$stage_dir/Lowfi.app/Contents/Resources/Lowfi.command"

"$script_dir/verify-app.sh" "$stage_dir/Lowfi.app" "$package_version"
mkdir -p "$(dirname "$bundle_dir")"
rm -rf "$bundle_dir"
mv "$stage_dir/Lowfi.app" "$bundle_dir"
echo "Built $bundle_dir"
