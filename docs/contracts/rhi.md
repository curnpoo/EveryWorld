# Contract: GPU interface (`ed_gpu`)

Owner: `ed_gpu`. Decisions: F7, P1-D1, P1-D2, P1-D3, P1-D4, P1-D8, P1-D9, P1-D10.
The interface itself is the Rust source: `crates/ed_gpu/src/lib.rs` (draft from Phase 0,
W4f). This document records why it has that shape, how each part maps to the three APIs,
and the tests. Change both together.

## 1. Shape

1. **Handles, not objects.** Buffers, textures, samplers, shaders, pipelines and surfaces
   are generational ids. No backend type is visible.
2. **Explicit resource states.** Every use of a texture or buffer is one `ResourceState`.
   The caller issues `transition`; the encoder never transitions by itself. The frame graph
   in `ed_render` computes transitions from declared pass inputs and outputs (P1-D3).
3. **Bindless by default.** The device owns one texture table and one buffer table. Shaders
   index them with ids from instance and material data. A draw sets a pipeline and 64 bytes
   of constants.
4. **No vertex layouts.** Vertex shaders pull from bindless buffers. This removes the
   largest area of difference between the APIs and suits GPU-driven drawing.
5. **Precompiled shaders.** The device takes Metal libraries or source, SPIR-V, or DXIL.
   Slang produces them at build time.
6. **Capability probe.** `Caps` is filled at creation; renderer features test its fields.
7. **One frame, one encoder** per thread, between `begin_frame` and `end_frame`; two or
   three frames in flight.

## 2. Walk against the three APIs

| Interface item | Metal | Vulkan | D3D12 | Consequence for the interface |
|---|---|---|---|---|
| `ResourceState` + `transition` | No states. Backend validates in debug, emits nothing | Image layouts and access masks; one `vkCmdPipelineBarrier2` per `transition` call | Resource states; `ResourceBarrier` | States must be per subresource (mip, layer) because Vulkan and D3D12 track them so. Buffers need states only for D3D12; kept for symmetry |
| `Undefined` as a source state | Free | `VK_IMAGE_LAYOUT_UNDEFINED` discards | Needs a discard or clear on first use of render targets | Kept: lets tile GPUs skip loads |
| `DepthRead` (test and sample at once) | Allowed | `DEPTH_READ_ONLY_OPTIMAL` | `DEPTH_READ` combined with shader resource | Needs one combined state, not two |
| Bindless tables | Argument buffers tier 2; resources must also be marked used (`useResource` or residency sets) | Descriptor indexing: partially bound, update-after-bind, runtime arrays | Shader-visible descriptor heaps, or `ResourceDescriptorHeap[]` | The device owns the tables and the per-frame "mark used" list. Table sizes come from `Caps`. Separate tables for textures and buffers |
| Samplers | Sampler states in an argument buffer | Separate sampler descriptors | Sampler heap limited to 2,048 | A small fixed set of samplers bound by the device, not per-material samplers |
| Draw constants (64 bytes) | No push constants: `setVertexBytes` / `setFragmentBytes` | Push constants (128 bytes guaranteed) | Root constants | 64 bytes fits all three |
| `draw_indexed_indirect_multi` with count | No multi-draw with count. Either an indirect command buffer filled by compute, or a CPU loop of `drawIndexedPrimitives(indirectBuffer:)` | `vkCmdDrawIndexedIndirectCount` (core in 1.2) | `ExecuteIndirect` with a count buffer | The count is optional and `Caps::indirect_count` says whether it is honoured. **Entries past the count must have zero instances.** The culling shader writes compacted entries and zeroes the rest |
| `DrawIndexedArgs` layout | 5 x `u32`, same order | Same | Same | One struct, no translation |
| 32-bit indices only | Supported | Supported | Supported | Simplifies meshes and clusters |
| Render pass with load and store ops | Render pass descriptor | Dynamic rendering (1.3) | Render passes, or `OMSetRenderTargets` plus clears | Passes are begin and end pairs with attachments; no framebuffer or render-pass objects |
| Reverse-Z, 32-bit float depth, infinite far | Supported | Supported; clip depth 0 to 1 natively | Supported | Clip space is 0 to 1 depth on all three; **Y is flipped on Vulkan**, handled by a negative viewport height inside the backend |
| Compute and `dispatch_indirect` | Supported (`dispatchThreadgroups(indirectBuffer:)`) | Supported | Supported | None |
| Shader write to textures in fragment and compute | Read-write textures by tier | Storage images | Unordered access views | `ShaderWrite` is one state; format support is probed |
| 64-bit atomics on buffers | Compiles on the M3 Pro at Metal language 3.0 to 4.0 | `shaderInt64` plus buffer atomics through SPIR-V; 64-bit atomics are not universal | 64-bit interlocked operations need a recent shader model | A `Caps` flag; the software rasteriser is optional (Track G) |
| Staged uploads (`write_buffer`, `write_texture`) | Blit from a staging buffer; shared memory on Apple silicon | Staging buffer and copy | Upload heap and copy | The device owns staging and applies writes at frame start; the caller budgets bytes per frame |
| Per-frame CPU-written buffers | Shared storage, one copy per frame in flight | Persistently mapped, one per frame | Upload heap, one per frame | `fill_frame_buffer` hides the per-frame copies |
| Readback | Blit to shared buffer, read after completion | Copy to a mapped buffer | Readback heap | `read_buffer` returns the last finished frame that wrote it: picking and screenshots are a few frames late by contract |
| GPU timers | Counter sample buffers (timestamps at pass boundaries on Apple GPUs) | Timestamp queries | Timestamp queries | Timers are scopes around passes, not around single draws; results arrive late through `timings` |
| Presentation | `CAMetalLayer` drawable | Swapchain; can be out of date | DXGI flip-model swapchain | `acquire` can return `SurfaceLost`; the caller reconfigures. HDR and vsync are surface configuration |
| Device loss | Rare | `VK_ERROR_DEVICE_LOST` | Device removed | `Error::DeviceLost`; the app recreates the device and reloads from packs |
| Deferred destruction | ARC, but in-flight use must be respected | Manual | Manual | `destroy_*` defers by frames in flight, on every backend |
| Mesh shaders, ray tracing, sparse textures | Available on the M3 Pro | Extensions | Optional features | `Caps` flags only; no interface for them until a phase needs one |

Items the draft deliberately leaves out, with the phase that may add them: ray-tracing
acceleration structures (Phase 4, if hardware GI is chosen), sparse or virtual texture
binding (Phase 2), multi-threaded command recording (when a profile asks for it), MSAA
resolve (not planned; the renderer uses temporal upscaling).

## 3. Second backend (F7, P1-D22)

**Vulkan**, through `ash`.

- It can run **on the development Mac** through MoltenVK with the validation layers on, so
  every state, layout and descriptor mistake is caught on the machine where the code is
  written. D3D12 cannot run on a Mac at all.
- It is the stricter of the two about image layouts and render-pass rules, which is the
  purpose of the second backend.
- It also covers Linux servers that render thumbnails, Steam Deck and Android later.
- D3D12 remains the Windows shipping backend and is written in Phase 9 against an interface
  that has already been proven on two APIs.

Limit to state plainly: Vulkan on a Mac is still Metal underneath, so driver behaviour and
performance on Windows and Linux hardware are not tested by this choice. It tests the
interface, which is what F7 asks for.

## 4. Invariants

1. No backend type, pointer or enum appears in the public API except `WindowHandle`.
2. A resource is used only in a state allowed by its `Usage`, and only in the state the
   command needs. With validation on, a violation is an error on every backend, including
   Metal, where the API itself would not notice.
3. Handles are generational. A stale handle is an `Invalid` error, never undefined behaviour.
4. Bindless indices are stable from `bind_*` to `unbind_*` and are not reused until every
   frame in flight that could reference them has finished.
5. Nothing blocks on the GPU except `begin_frame` (frame pacing) and `wait_idle`.
6. The same command stream produces the same image on every backend, within a tolerance
   set by the image tests.

## 5. Tests

| # | Test |
|---|---|
| R1 | Handle lifetime: use after destroy is `Invalid`; destroy during frames in flight is deferred |
| R2 | State validation: each wrong-state use fails on Metal with validation on, and on Vulkan under the validation layers with zero layer messages for the correct sequence |
| R3 | Bindless: 10,000 textures bound, sampled by index, unbound and rebound; no stale reads |
| R4 | `draw_indexed_indirect_multi` with and without count draws the same image; entries past the count are zeroed by the reference culling shader |
| R5 | Compute writes a buffer, a later pass reads it; `dispatch_indirect` |
| R6 | Upload budget: writes above the per-frame budget are spread over frames; no frame exceeds it |
| R7 | Readback: a picking buffer returns the expected id within three frames |
| R8 | Timers: nested scopes return plausible, ordered times |
| R9 | Resize and surface loss: 100 resizes with no leak and no validation message |
| R10 | Image tests: the proving-ground scene matches a reference image on Metal and on Vulkan |
| R11 | Capability probe: the M3 Pro reports the values recorded in `../reference/spikes/w4b-geometry.md` |
