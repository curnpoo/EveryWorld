#!/bin/bash
set -e
cd "$(dirname "$0")/.."
cargo build --release -p ed_game -p ed_server
APP="target/release/EveryWorld.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/earthdrop "$APP/Contents/MacOS/earthdrop"
cp target/release/earthdrop-server "$APP/Contents/MacOS/earthdrop-server"
cp packaging/Info.plist "$APP/Contents/Info.plist"
# ad-hoc sign so Gatekeeper doesn't block it as hard
codesign --force --deep --sign - "$APP" 2>/dev/null || true
# zip for sending (preserves exec bit, avoids quarantine issues on unzip)
cd target/release
zip -r EveryWorld-mac.zip EveryWorld.app
echo "Done: target/release/EveryWorld-mac.zip"
echo "Send that zip to your friend. They unzip, right-click EveryWorld.app -> Open (first time), then run."
