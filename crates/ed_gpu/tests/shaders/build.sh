#!/bin/sh
# Regenerates spv/*.spv from the GLSL test shaders (glslangValidator from the Vulkan SDK or
# Homebrew `glslang`). The shaders include ../../src/shader_abi.glsl, the ed_gpu binding ABI.
# Checked in so `cargo test` needs no shader compiler; tests/shaders_fresh.rs checks they match.
set -eu
cd "$(dirname "$0")"
mkdir -p spv
for f in *.vert *.frag *.comp; do
    glslangValidator -V --target-env vulkan1.3 -I../../src "$f" -o "spv/$f.spv" >/dev/null
    if command -v spirv-val >/dev/null; then spirv-val --target-env vulkan1.3 "spv/$f.spv"; fi
done
echo "compiled $(ls spv | wc -l | tr -d ' ') shaders"
