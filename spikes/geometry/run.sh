#!/usr/bin/env bash
# Usage: ./run.sh path/to/character.glb
set -euo pipefail
cd "$(dirname "$0")"
[ -d meshoptimizer ] || git clone --depth 1 https://github.com/zeux/meshoptimizer meshoptimizer
c++ -O2 -std=c++17 -I. clusterlod_spike.cpp meshoptimizer/src/*.cpp -o clusterlod_spike
python3 export_mesh.py "$1" character.bin
python3 export_mesh.py meshoptimizer/demo/pirate.obj pirate.bin
python3 export_mesh.py --grid 700 grid.bin
./clusterlod_spike character.bin pirate.bin grid.bin
