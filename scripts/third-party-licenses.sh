#!/bin/sh
# Writes the licences of all third-party crates (cargo-about, see about.toml)
# plus the NOTICE files some of them ship, e.g. the font licences of
# typst-assets, to the given file. Used by the release workflow.
#   scripts/third-party-licenses.sh THIRD-PARTY-LICENSES.txt
set -eu
cd "$(dirname "$0")/.."
out=${1:?output file}

cargo about generate --locked about.hbs -o "$out"

cargo metadata --format-version 1 --locked | python3 -c '
import json, os, sys
for p in sorted(json.load(sys.stdin)["packages"], key=lambda p: p["name"]):
    if p["source"] is None:
        continue  # DevMark itself
    d = os.path.dirname(p["manifest_path"])
    for f in sorted(os.listdir(d)):
        if f.upper().startswith("NOTICE"):
            print("-" * 80)
            print(p["name"], p["version"] + ":", f + "\n")
            print(open(os.path.join(d, f), encoding="utf-8").read())
' >> "$out"

# JetBrains Mono, built into the app (SIL Open Font License 1.1).
{
    echo "--------------------------------------------------------------------------------"
    echo "JetBrains Mono 2.304 (font, built into DevMark):"
    echo
    cat apps/desktop/assets/fonts/OFL-JetBrainsMono.txt
} >> "$out"
