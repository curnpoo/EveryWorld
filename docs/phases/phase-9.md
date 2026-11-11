# Phase 9 — Platforms and ship

Status: last phase. Roadmap row: `../ROADMAP.md`. Design: `../ARCHITECTURE.md` §11,
`../contracts/rhi.md`. Decisions in the P9-D series are **proposed**.

## 1. Goal

A product people can install: macOS and Windows builds that pass performance gates, a
proven mobile tier, signed and packaged, with every data and asset licence satisfied.

## 2. What Phase 9 proves

| Question | Proof |
|---|---|
| Was the RHI the right shape? | A full D3D12 backend passes the RHI suite and the reference scenes without changing the public interface |
| Does the Low tier scale down to a phone? | The golden path on an iPhone-class device inside a mobile budget |
| Is the build legally shippable? | `EW_GATE_SHIP=1` passes; ODbL position reviewed by counsel; `CREDITS.md` complete |
| Can it be delivered and updated? | Install, launch, patch and roll back on both desktop platforms |

## 3. What Phase 9 does not do

No new gameplay. No consoles. No engine changes that are not required by a platform.

## 4. Decisions in detail (proposed)

### P9-D1 — D3D12 backend
Built on the interface the Vulkan backend proved in Phase 1 (F7): descriptor heaps for the
bindless tables, explicit states from the frame graph, shaders from the single source
(P1-D1). It must reach feature parity with Metal on High, including VSM and GI where
adopted. The Vulkan backend is brought from minimal to complete only as far as Android needs.

### P9-D2 — Upscalers and ray tracing on Windows
DLSS and FSR behind the `Upscaler` interface of P4-D11; DXR for the Ultra reflections of
P4-D6. Each is a caps flag with a cost row.

### P9-D3 — Tiers by probe
`Caps` selects a starting tier per device; the player may change it. Tiers are data: a
table of flags, budgets and resolutions per platform class.

### P9-D4 — iOS
The Metal backend with the Low tier: baseline lighting, lower page resolution, smaller
residency budget, touch controls. Memory is the constraint; the budgeted selector of P2-D4
is what makes it possible.

### P9-D5 — Android
Vulkan on a current flagship class as the proof; broad device coverage is after ship.

### P9-D6 — Packaging
macOS: signed, notarised app bundle. Windows: signed installer. The globe pack ships with
the app; further content comes from the tile service behind a CDN. Updates are content
addressed: a patch is the set of new chunk hashes plus a new binary, so rollback is a
manifest switch.

### P9-D7 — Crash and performance reports
Opt-in, with no personal data beyond what contract rule 7 allows; symbolicated on our side.

### P9-D8 — Licence gate is strict
`EW_GATE_SHIP=1` on every release build: every dataset pinned and GREEN or reviewed
YELLOW, RED absent, attribution generated, Fab assets only inside packs (D8). The ODbL
position and the JPL element table are reviewed by counsel before any public build
(Phase 0 carried items).

### P9-D9 — Performance gates in CI
Self-hosted runners (a Mac, a Windows PC) run the harness on the reference scenes per
release candidate; a regression over a set margin against the stored budgets fails the
build (P1-D15: never hosted CI).

### P9-D10 — Product basics
Settings, key rebinding, controller support, subtitles and text scale, colour-blind-safe
UI palette, localisation plumbing with English first.

## 5. Budgets

| Platform class | Target |
|---|---|
| Mac, M3 Pro, 1440p | High at 60 fps (unchanged) |
| Mac, M1, 1080p | Low at 60 fps |
| Windows, mid-range discrete GPU, 1440p | High at 60 fps with an upscaler |
| Windows, integrated GPU, 1080p | Low at 30 fps |
| iPhone class with 8 GB | Low at 30 fps, resident memory under 3 GB, no thermal shutdown in a 20-minute session |
| Install size, desktop | Binary plus the globe pack (2.1 GiB budget, P0-R2) plus kit and material packs; reported |
| Cold launch to menu | 5 s on desktop |
| Patch for a code-only change | Binary size only |

Windows and mobile figures are set against hardware the owner names; they are confirmed by
the first measurement on each.

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | D3D12 backend | `ed_gpu/src/d3d12*` | RHI suite and reference scenes on Windows |
| W2 | Windows upscalers and ray tracing | `ed_render`, `ed_gpu` | Flags measured in the tier table |
| W3 | Tier tables and probe | `data/tiers/`, `ed_gpu` caps | Correct starting tier on each test device |
| W4 | iOS build and Low tier | `ew` platform layer | Golden path on device |
| W5 | Android proof | `ew` platform layer, Vulkan backend completion | Reference scenes on one device |
| W6 | Packaging, signing, updater | `packaging/`, `tools/` | Install, patch, rollback on both desktops |
| W7 | Tile service hosting and CDN | `ed_tiles`, deployment | Cold region load from the public endpoint inside the Phase 3 budget plus network |
| W8 | Licence, attribution, legal | `xtask`, `data/registry`, `CREDITS.md` | Strict gate passes; counsel sign-off recorded |
| W9 | Performance gates on runners | CI, `tools/` | A seeded regression fails the build |
| W10 | Product basics | `ew`, `ed_ui` | Checklist of P9-D10 |
| W11 | Crash reporting | `ew`, a small service | A forced crash arrives symbolicated |

## 7. Order and milestones

```
W1 ──> W2 ──┐
W3 ─────────┼──> W9 ──> W6 ──> release candidate ──> gate
W8 ─────────┤
W4, W5 ─────┘ (mobile proof does not block the desktop release candidate)
W7, W10, W11 alongside
```

**M23. Windows native.** W1 to W3. *Gate:* reference scenes at budget on D3D12.
**M24. Shippable desktop.** W6 to W11. *Gate:* release candidates for macOS and Windows
pass the strict licence gate and the performance gates.
**M25. Mobile proof.** W4, W5. *Gate:* the golden path on an iOS device.
**M26. Ship.** *Gate:* the Phase 9 exit gate.

## 8. Exit gate

- [ ] Ships on macOS and Windows.
- [ ] Mobile tier proven.
- [ ] Strict licence gate passing; legal review recorded.
- [ ] Performance gates enforced on self-hosted runners.
- [ ] Install, patch and rollback verified.

## 9. Failure branches

| If | Then |
|---|---|
| D3D12 exposes an RHI flaw late | Fix the interface; the Vulkan suite catches regressions on the Mac |
| VSM or GI miss budget on Windows hardware | They stay per-tier flags; the baseline ships |
| iOS memory cannot hold the Low tier | Lower residency and page size in the tier table; ship fewer levels locally |
| Counsel rejects the ODbL position | Ship the affected layers only through the tile service with the required notices, or replace the source; no public build before this is settled |
| Notarisation or signing blocks | Release is held; no unsigned public build |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Legal review of ODbL and the JPL table | Owner with counsel |
| Developer accounts and signing certificates for Apple and Windows | Owner |
| Test hardware: a Windows PC per class, an iPhone, an Android device | Owner |
| Product name, store presence, public playtest timing | Owner (open questions in `DECISIONS.md`) |
| CDN and hosting account | Owner |
