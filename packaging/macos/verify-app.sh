#!/bin/sh
set -eu

bundle=${1:?usage: verify-app.sh BUNDLE VERSION}
version=${2:?usage: verify-app.sh BUNDLE VERSION}
plist="$bundle/Contents/Info.plist"
macos="$bundle/Contents/MacOS"
resources="$bundle/Contents/Resources"

[ -d "$bundle" ]
[ -x "$macos/lowfi" ]
[ -x "$macos/lowfi-app-launcher" ]
[ -x "$resources/Lowfi.command" ]
[ -s "$resources/Lowfi.icns" ]
plutil -lint "$plist"

expect() {
    key=$1
    expected=$2
    actual=$(plutil -extract "$key" raw -o - "$plist")
    [ "$actual" = "$expected" ] || {
        echo "$key is $actual; expected $expected" >&2
        exit 1
    }
}

expect CFBundleDisplayName Lowfi
expect CFBundleExecutable lowfi-app-launcher
expect CFBundleIconFile Lowfi.icns
expect CFBundleIdentifier io.github.talwat.lowfi
expect CFBundleInfoDictionaryVersion 6.0
expect CFBundleName Lowfi
expect CFBundlePackageType APPL
expect CFBundleShortVersionString "$version"
expect CFBundleVersion "$version"
"$macos/lowfi" --help >/dev/null
"$macos/lowfi" --version >/dev/null
"$resources/Lowfi.command" --help >/dev/null

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT HUP INT TERM
iconutil -c iconset "$resources/Lowfi.icns" -o "$temporary/Lowfi.iconset"
relocated="$temporary/Lowfi café's copy.app"
cp -R "$bundle" "$relocated"
"$relocated/Contents/Resources/Lowfi.command" --help >/dev/null
mv "$relocated/Contents/MacOS/lowfi" "$relocated/Contents/MacOS/lowfi.missing"
if "$relocated/Contents/Resources/Lowfi.command" --help >/dev/null 2>&1; then
    echo "Lowfi.command succeeded without its executable" >&2
    exit 1
fi
