#!/usr/bin/env bash
#
# Build Blink Reminder as a double-clickable macOS .app bundle in ./dist.
#
# Usage:
#   ./package-macos.sh                 # native build for this Mac
#   ./package-macos.sh --universal     # universal (arm64 + x86_64) binary
#   ./package-macos.sh --dmg           # also produce a shareable .dmg
#   ./package-macos.sh --universal --dmg
#
set -euo pipefail
cd "$(dirname "$0")"

APP_NAME="Blink Reminder"
BIN_NAME="blink-rust"
BUNDLE_ID="com.blink.reminder"
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"

UNIVERSAL=0
DMG=0
for arg in "$@"; do
    case "$arg" in
        --universal) UNIVERSAL=1 ;;
        --dmg) DMG=1 ;;
        *) echo "unknown option: $arg" >&2; exit 1 ;;
    esac
done

if [[ $UNIVERSAL -eq 1 ]]; then
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

# Build the .icns app icon from the master PNG (sips + iconutil are built in).
ICON_PLIST=""
if [[ -f assets/icon.png ]]; then
    echo "Building app icon…"
    ICONSET="$(mktemp -d)/AppIcon.iconset"
    mkdir -p "$ICONSET"
    for sz in 16 32 128 256 512; do
        sips -z "$sz" "$sz" assets/icon.png --out "$ICONSET/icon_${sz}x${sz}.png" >/dev/null
        sips -z "$((sz * 2))" "$((sz * 2))" assets/icon.png \
            --out "$ICONSET/icon_${sz}x${sz}@2x.png" >/dev/null
    done
    iconutil -c icns "$ICONSET" -o "$APP_DIR/Contents/Resources/AppIcon.icns"
    rm -rf "$(dirname "$ICONSET")"
    ICON_PLIST="    <key>CFBundleIconFile</key><string>AppIcon</string>"
else
    echo "warning: assets/icon.png missing; building without an icon" >&2
fi

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
${ICON_PLIST}
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

if [[ $DMG -eq 1 ]]; then
    DMG_PATH="dist/${APP_NAME}.dmg"
    echo "Building ${DMG_PATH}…"
    STAGING="$(mktemp -d)"
    cp -R "$APP_DIR" "$STAGING/"
    ln -s /Applications "$STAGING/Applications"   # drag-to-install target
    rm -f "$DMG_PATH"
    hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" \
        -ov -format UDZO "$DMG_PATH" >/dev/null
    rm -rf "$STAGING"
    echo "Done: ${DMG_PATH}"
fi

echo "Run it with:  open \"${APP_DIR}\"    (look for the eye icon in the menu bar)"
