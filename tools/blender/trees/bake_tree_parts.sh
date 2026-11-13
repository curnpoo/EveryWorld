#!/bin/sh
# Bakes the tree parts (trees G6) into the cache the game reads: textures, bark, parts glTF.
#   tools/blender/trees/bake_tree_parts.sh [OUT]   (default $EARTHDROP_CACHE/tree_parts, else
#   .earthdrop-cache/tree_parts). Set BLENDER to override the Blender binary.
set -e
cd "$(dirname "$0")/../../.."
OUT=${1:-${EARTHDROP_CACHE:-.earthdrop-cache}/tree_parts}
BLENDER=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}
"$BLENDER" --background --factory-startup --python tools/blender/trees/build_tree_parts.py -- --out "$OUT" 2>&1 | grep -E '^tree_parts|Error|Traceback|line [0-9]' || true
test -f "$OUT/tree_parts.glb" && test -f "$OUT/manifest.json" && echo "tree parts in $OUT"
