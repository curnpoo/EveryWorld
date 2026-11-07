# Windows: build, test, play

EveryWorld runs on Windows today through **Bevy's wgpu renderer** (wgpu picks
D3D12 or Vulkan from your GPU driver). The native Metal renderer is macOS-only:
`ed_game::native::enabled()` is false here, so `EW_NATIVE` is ignored and the
window is a normal opaque one. Same world, same gameplay, same CLI as macOS —
this is exactly the look a Mac produces with `EW_NATIVE=0`.

## One-time setup

1. **Rust (MSVC toolchain)** — install from <https://rustup.rs>. The default
   `stable-x86_64-pc-windows-msvc` is what you want.
2. **Visual Studio Build Tools 2022** with the *Desktop development with C++*
   workload and a Windows SDK. This provides `link.exe`/`cl.exe` for the
   `cc`-built crates (`meshopt`, `ring`). No Python, NASM or Perl needed:
   `ring` uses the pre-generated objects shipped in its crates.io package.
3. **Git for Windows** (for clone/pull; `.gitattributes` keeps checkouts
   byte-identical across platforms).

No GPU SDK install is required: D3D12 is part of Windows, Vulkan comes from the
GPU driver where present.

## Everyday commands (PowerShell, repo root)

```powershell
cargo test --workspace            # fast, offline unit tests
cargo build --release             # -> target\release\earthdrop.exe
cargo run --release
```

First run needs network: `cargo test` fetches crates and the game streams map
data into its cache. Location selection happens in the UI. Paths:

| What | Windows | macOS |
|---|---|---|
| Settings | `%APPDATA%\EveryWorld\settings.json` | `~/Library/Application Support/EveryWorld/settings.json` |
| Tile/data cache | `%LOCALAPPDATA%\earthdrop` | `~/Library/Caches/earthdrop` |
| Overrides | `EW_SETTINGS=path`, `EARTHDROP_CACHE=path` | same |

The cache layout is plain keyed files, so copying the Mac's
`~/Library/Caches/earthdrop` folder to `%LOCALAPPDATA%\earthdrop` skips the
first-run downloads (tiles are immutable; only weather may refetch).

## Seeing logs (GPU adapter, fps, captures)

Bevy's default filter hides info lines, including the GPU-adapter report. Set:

```powershell
$env:RUST_LOG = "info,wgpu=error,naga=warn,bevy_render=warn"
```

For an in-game fps overlay: `$env:EW_GFX = "stats=1"`.

### GPU adapter probe (Dx12 or Vulkan + GPU name)

```powershell
$env:RUST_LOG = "info,wgpu=error,naga=warn,bevy_render=warn"
$env:EW_PERF = "1"; $env:EW_PERF_OUT = "target\adapter.csv"
cargo run --release -- --headless --fly --lat 40.77 --lon -73.97 --alt 300 --width 640 --height 360 --quit-after 10 *> target\adapter.log
Select-String -Path target\adapter.log -Pattern "GPU adapter|panicked|ERROR"
```

Report the full `GPU adapter: ...` line — its `backend:` field is `Dx12` or
`Vulkan`.

### Screenshot smoke test

```powershell
cargo run --release -- --headless --fly --lat 40.77 --lon -73.97 --alt 300 --heading 20 --pitch -14 --hour 10.5 --width 1280 --height 720 --frames 60 --screenshot target\smoke.png
Get-Item target\smoke.png | Select-Object Name, Length
```

The first run streams tiles before the capture (up to ~8 minutes worst case,
usually far less); later runs are fast. A settled near-ground shot is hundreds
of KB (300 KB–2 MB measured on macOS) — a file of only a few KB means a black
or empty frame. Drop `--headless` for a windowed shot (keep the window visible;
a minimized window may not render).

### Six-scene perf battery (mirrors `tools/perf_headless.sh`)

```powershell
$env:RUST_LOG = "info,wgpu=error,naga=warn,bevy_render=warn"
$env:EW_PERF = "1"
$inv = [System.Globalization.CultureInfo]::InvariantCulture
$out = "target\win-perf"; New-Item -ItemType Directory -Force -Path $out | Out-Null
cargo build --release
$exe = "target\release\earthdrop.exe"
$common = @("--headless","--fly","--width","2144","--height","1206","--hour","15","--weather","fair")
$scenes = [ordered]@{
  "still"  = @("--place","Zermatt","--alt","120","--pitch","-12","--quit-after","22")
  "cruise" = @("--place","Zermatt","--alt","120","--pitch","-12","--cruise","60","--quit-after","25")
  "spin"   = @("--place","Zermatt","--alt","20","--pitch","-5","--spin","30","--quit-after","25")
  "forest" = @("--lat","48.0","--lon","8.2","--alt","3000","--pitch","-25","--quit-after","22")
  "lapaz"  = @("--lat","-16.5","--lon","-68.15","--alt","300","--pitch","5","--quit-after","22")
  "origin" = @("--place","Zermatt","--alt","400","--pitch","-15","--cruise","400","--quit-after","25")
}
foreach ($s in $scenes.Keys) {
  $env:EW_PERF_OUT = Join-Path $out "$s.csv"
  $log = Join-Path $out "$s.log"
  $argList = $common + $scenes[$s]
  Write-Host "== $s"
  & $exe @argList *> $log
  if ($LASTEXITCODE -ne 0) { Write-Host "  !! $s exit code $LASTEXITCODE" }
  $rows = @(Import-Csv (Join-Path $out "$s.csv"))
  if ($rows.Count -eq 0) { Write-Host "  !! $s produced no perf rows"; continue }
  $load = $rows.Count
  for ($i = 0; $i -lt $rows.Count; $i++) { if ([int]$rows[$i].pending -eq 0) { $load = $i; break } }
  if ($load + 1 -lt $rows.Count) { $after = @($rows[($load+1)..($rows.Count-1)]) } else { $after = @($rows[$rows.Count-1]) }
  $fps = @($after | ForEach-Object { [double]::Parse($_.fps, $inv) })
  $p99 = @($after | ForEach-Object { [double]::Parse($_.p99_ms, $inv) })
  $avg = ($fps | Measure-Object -Average).Average
  $min = ($fps | Measure-Object -Minimum).Minimum
  $worstp99 = ($p99 | Measure-Object -Maximum).Maximum
  Write-Host ("  {0}: fps avg {1:N1} min {2:N1} | p99 worst {3:N1} ms" -f $s, $avg, $min, $worstp99)
}
Select-String -Path (Join-Path $out "*.log") -Pattern "panicked|ERROR" | Select-Object -First 20
Remove-Item Env:RUST_LOG, Env:EW_PERF, Env:EW_PERF_OUT -ErrorAction SilentlyContinue
```

Runtime is roughly 3–5 minutes (each scene self-quits via `--quit-after`).
`EW_PERF_OUT` keeps the tracked `perf.csv` at the repo root untouched.

## Handing this to a coding agent

Run these in PowerShell from the repo root, in order, and **paste all output**
(exact error lines, no paraphrasing):

1. `cargo test --workspace` — expect every suite green; paste the summaries.
2. `cargo build --release` — expect success; paste any errors.
3. GPU adapter probe above — paste the full `GPU adapter: ...` line.
4. Screenshot smoke above — report the PNG size and any `panicked`/`ERROR`.
5. Six-scene perf battery above — paste the per-scene fps lines.
6. Optional windowed play: `cargo run --release` —
   confirm menu, world streaming, movement and audio.

CI (`.github/workflows/ci.yml`) runs `cargo test --workspace` on
`macos-latest` and `windows-latest` for every push — check that first if the
machine is remote.

## What differs from macOS

- **Renderer:** Bevy/wgpu instead of native Metal. Screenshots are close but
  not pixel-identical to a default Mac session; for a same-renderer A/B run
  the Mac with `EW_NATIVE=0`.
- **Menu label:** "Upscaler" says *MetalFX*, but on the Bevy path it is the
  FSR1/EASU render-scale path — same role, different implementation.
- **Bevy-path knobs all work here:** `EW_BISECT`, `EW_POST`, `EW_GFX`,
  `EW_FSR`. On a default Mac (native renderer) several are no-ops, so Windows
  is a good platform for those A/B runs.
- **macOS-only tools:** `tools/gpu_passes.sh` needs `xcrun`/Xcode; use the
  `--perf` CSV here (or PIX/RenderDoc for per-pass GPU timing). The `.sh`
  batteries need bash + python3; the PowerShell snippets above replace them.
- **`EW_NATIVE=1` does nothing on Windows** — the native renderer and its
  switches (`EW_NATIVE_NOSSR`, MetalFX, ...) are compiled out.

## Troubleshooting

- `link.exe not found` / `LNK1181` → install VS Build Tools with the C++
  workload (setup step 2), then open a fresh terminal.
- `cargo test` needs network on the first run only; later:
  `cargo test --workspace --offline`.
- Tiles fail to download → check firewall/proxy, or copy the Mac cache as
  described above (`EARTHDROP_CACHE` can point at any copy).
- Screenshot missing or a few KB → use the `--headless` form; for windowed
  shots keep the window visible and un-minimized.
- Settings not persisting → check `%APPDATA%\EveryWorld\settings.json`
  exists after changing a slider; `EW_SETTINGS=path` overrides it.
- SmartScreen on a zipped `earthdrop.exe` from the internet → "More info →
  Run anyway". Locally built binaries are not affected.
