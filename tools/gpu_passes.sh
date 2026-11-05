#!/usr/bin/env bash
# Per-pass GPU time on Metal, no code changes: Metal System Trace on a headless run.
# Usage: tools/gpu_passes.sh OUT_DIR [perf_headless scene args...]   (default: Zermatt still)
# Prints busy ms/frame per encoder label, split into fragment / vertex / compute.
# Metal GPUs have no in-encoder timestamps through wgpu, so Bevy's own pass timers are empty;
# this reads the labelled encoders from the trace instead.
set -euo pipefail
out=${1:?usage: tools/gpu_passes.sh OUT_DIR [args...]}; shift || true
mkdir -p "$out"
args=("$@"); [ ${#args[@]} -eq 0 ] && args=(--place Zermatt --alt 120 --pitch -12)
bin=target/release/earthdrop
EW_PERF=1 EW_PERF_OUT="$out/perf.csv" "$bin" --headless --fly --width 2144 --height 1206 --hour 15 --weather fair "${args[@]}" --quit-after 40 >"$out/run.log" 2>&1 &
pid=$!
sleep 14   # let tiles load
rm -rf "$out/trace.trace"
xcrun xctrace record --template 'Metal System Trace' --attach "$pid" --time-limit 4s --output "$out/trace.trace" >/dev/null 2>&1
kill "$pid" 2>/dev/null || true
xcrun xctrace export --input "$out/trace.trace" --xpath '/trace-toc/run[1]/data/table[@schema="metal-gpu-intervals"]' --output "$out/gpu.xml" >/dev/null 2>&1
python3 -I tools/gpu_sum.py "$out/gpu.xml" "$out/perf.csv"
