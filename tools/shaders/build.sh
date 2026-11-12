#!/bin/sh
# Regenerates every checked-in shader output from its Slang source (P1-D1): MSL for Metal and
# SPIR-V for Vulkan under <dir>/gen/ for crates/ed_render/shaders, crates/ed_editor/shaders and
# crates/ed_gpu/tests/shaders/slang. Needs slangc 2026.20 (SLANGC=<path>, PATH, or
# ~/.local/opt/slang-2026.20/bin/slangc); spirv-val is used when on PATH.
# The freshness tests (ed_gpu tests/slang_fresh.rs) fail until this is re-run after a change.
set -eu
cd "$(dirname "$0")/../.."
exec cargo run --quiet --release -p ed_gpu --example slang_gen -- "$@"
