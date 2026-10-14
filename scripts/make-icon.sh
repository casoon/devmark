#!/bin/sh
# Erzeugt apps/desktop/assets/AppIcon.icns aus AppIcon.typ (braucht das typst-CLI und iconutil).
set -eu
cd "$(dirname "$0")/../apps/desktop/assets"
work=$(mktemp -d)
typst compile AppIcon.typ "$work/icon.png" --ppi 72
mkdir "$work/AppIcon.iconset"
for size in 16 32 128 256 512; do
  sips -z $size $size "$work/icon.png" --out "$work/AppIcon.iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$work/icon.png" --out "$work/AppIcon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$work/AppIcon.iconset" -o AppIcon.icns
rm -r "$work"
echo "AppIcon.icns erzeugt"
