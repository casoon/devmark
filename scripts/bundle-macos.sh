#!/bin/sh
# Baut DevMark.app (Release) unter target/release/bundle/ und installiert optional nach /Applications.
#   scripts/bundle-macos.sh            # nur bauen
#   scripts/bundle-macos.sh --install  # bauen und nach /Applications kopieren
set -eu
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' apps/desktop/Cargo.toml | head -1)
cargo build --release -p devmark

app=target/release/bundle/DevMark.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/devmark "$app/Contents/MacOS/devmark"
strip -x "$app/Contents/MacOS/devmark" 2>/dev/null || true
cp apps/desktop/assets/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DevMark</string>
  <key>CFBundleDisplayName</key><string>DevMark</string>
  <key>CFBundleIdentifier</key><string>de.casoon.devmark</string>
  <key>CFBundleExecutable</key><string>devmark</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleDevelopmentRegion</key><string>de</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Markdown</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string></array>
      <key>CFBundleTypeExtensions</key><array><string>md</string><string>markdown</string><string>mdx</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Typst</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>CFBundleTypeExtensions</key><array><string>typ</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Text</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>public.plain-text</string><string>public.source-code</string><string>public.folder</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

# Ad-hoc-Signatur: lokal gebaute App startet ohne Gatekeeper-Rückfrage.
codesign --force --deep --sign - "$app"
echo "Gebaut: $app"

if [ "${1:-}" = "--install" ]; then
  rm -rf /Applications/DevMark.app
  cp -R "$app" /Applications/DevMark.app
  # Finder/Launch Services über neue Dokumenttypen informieren.
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f /Applications/DevMark.app || true
  echo "Installiert: /Applications/DevMark.app"
fi
