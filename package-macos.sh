#!/usr/bin/env bash
#
# Build Blink Reminder as a double-clickable macOS .app bundle in ./dist.
#
# Usage:
#   ./package-macos.sh                 # native build for this Mac
#   ./package-macos.sh --universal     # universal (arm64 + x86_64) binary
#
set -euo pipefail
cd "$(dirname "$0")"

APP_NAME="Blink Reminder"
BIN_NAME="blink-rust"
BUNDLE_ID="com.blink.reminder"
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"

if [[ "${1:-}" == "--universal" ]]; then
    echo "Building universal release binary…"
    rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    BIN_PATH="target/${BIN_NAME}-universal"
    lipo -create -output "$BIN_PATH" \
        "target/aarch64-apple-darwin/release/${BIN_NAME}" \
        "target/x86_64-apple-darwin/release/${BIN_NAME}"
else
    echo "Building release binary…"
    cargo build --release
    BIN_PATH="target/release/${BIN_NAME}"
fi

APP_DIR="dist/${APP_NAME}.app"
echo "Assembling ${APP_DIR}…"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/${BIN_NAME}"
chmod +x "$APP_DIR/Contents/MacOS/${BIN_NAME}"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleExecutable</key><string>${BIN_NAME}</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>10.15</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

# Ad-hoc codesign so Gatekeeper/macOS is happy launching a local build.
if command -v codesign >/dev/null; then
    codesign --force --deep --sign - "$APP_DIR" >/dev/null 2>&1 || true
fi

echo "Done: ${APP_DIR}"
echo "Run it with:  open \"${APP_DIR}\"    (look for the eye icon in the menu bar)"
