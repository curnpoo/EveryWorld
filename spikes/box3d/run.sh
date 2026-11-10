#!/usr/bin/env bash
# Fetch, build and run the Box3D spike. Usage: ./run.sh [extra cmake flags]
set -euo pipefail
cd "$(dirname "$0")"
[ -d src ] || git clone --depth 1 https://github.com/erincatto/box3d src
cmake -S src -B build -DCMAKE_BUILD_TYPE=Release -DBOX3D_SAMPLES=OFF -DBOX3D_UNIT_TESTS=OFF -DBOX3D_VALIDATE=OFF "$@" >/dev/null
cmake --build build -j 8 >/dev/null
LIB=$(find build -name 'libbox3d*.a' | head -1)
cc -O2 -std=c17 -Isrc/include spike.c "$LIB" -lm -o spike
./spike
