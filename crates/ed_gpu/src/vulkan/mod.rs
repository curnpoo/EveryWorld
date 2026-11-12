//! Vulkan backend (P1-D22, M4b-2). Private: nothing here is visible outside the crate.
//!
//! The second backend exists to prove the RHI is not Metal-shaped. It is minimal, not fast:
//! one dedicated `VkDeviceMemory` per resource, one queue, one primary command buffer per frame.
//! On macOS it runs on MoltenVK (portability enumeration).
//!
//! Design notes:
//! - All mutable state lives in one `Mutex<State>`, as on Metal. Command recording uses the
//!   frame's own command buffer without the lock and locks briefly to resolve handles and
//!   update tracked states.
//! - Bindless: descriptor indexing (P1-D9). One descriptor set layout with four
//!   update-after-bind, partially-bound bindings (sampled images, storage images, storage
//!   buffers, samplers) and one set per frame slot, so a `cpu_write` buffer can point at its
//!   per-slot copy. Every pass binds the slot's set and pushes 64 bytes of zero constants.
//!   Slang shaders (the engine's, `shader_abi.slang`) read buffers through device addresses
//!   instead: binding 4 is a storage buffer holding one `u64` buffer device address per table
//!   index (per frame slot, like the descriptors), so `ed_buf<T>(i)` is a `T*` on both Metal
//!   and Vulkan. The storage-buffer array at binding 2 stays for hand-written GLSL.
//! - Freed table slots are overwritten with dummy descriptors when they retire (MoltenVK keeps
//!   pointers to the objects in its descriptors, so a stale one must not outlive its object).
//! - States: the tracker in `crate::track` drives barriers. Every transition becomes one
//!   `vkCmdPipelineBarrier2` built from the *tracked* source state (see `conv::access`).
//! - Frames: one timeline semaphore; frame `f` signals `f + 1`.
//! - Timers: timestamp queries at scope boundaries (`TimerSource::PassBoundary`).

mod conv;
mod encoder;

use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use ash::vk;
use ash::vk::Handle;

use crate::pool::{Pool, Slots};
use crate::track::States;
use crate::*;

/// Upper bounds on table sizes (clamped to the device's limits).
const TEXTURE_TABLE: u32 = 1 << 17;
const BUFFER_TABLE: u32 = 1 << 16;
const SAMPLER_TABLE: u32 = 4096;
/// Smallest texture or buffer table the backend accepts before giving up on the device.
const MIN_TABLE: u32 = 1024;
/// Timestamp queries per frame (two per timer scope).
const MAX_QUERIES: u32 = 2048;

const B_TEXTURES: u32 = 0;
const B_IMAGES: u32 = 1;
const B_BUFFERS: u32 = 2;
const B_SAMPLERS: u32 = 3;
/// Buffer address table (Slang ABI): one `u64` device address per buffer table index, one
/// table per frame slot (a `cpu_write` buffer's entry is that slot's copy).
const B_ADDRS: u32 = 4;

/// Validation messages that are expected on MoltenVK and do not indicate a bug here.
/// (message id name, reason). Printed once with the reason instead of failing the frame.
/// Applied only when the device's driver is MoltenVK.
const WAIVED: &[(&str, &str)] = &[(
    "VUID-vkCreateDescriptorSetLayout-support-09582",
    "MoltenVK answers vkGetDescriptorSetLayoutSupport from its non-argument-buffer maxPerSetDescriptors (1212) \
     while its update-after-bind limits (1,000,000 per binding) allow the bindless tables; the set works through \
     Metal argument buffers",
)];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemKind {
    Gpu,
    Upload,
    Readback,
}

/// One `VkBuffer` with its own memory; `ptr` is the persistent mapping (null if not mapped).
pub(crate) struct RawBuffer {
    raw: vk::Buffer,
    mem: vk::DeviceMemory,
    ptr: *mut u8,
    bytes: u64,
}

// SAFETY: `ptr` points into a persistent mapping owned by `mem`; writes through it happen only
// under the device's rules (fill_frame_buffer on the render thread, staging by the encoder that
// owns the frame slot), reads only after the GPU finished.
unsafe impl Send for RawBuffer {}
// SAFETY: as above.
unsafe impl Sync for RawBuffer {}

pub(crate) struct Buffer {
    /// One per frame in flight for `cpu_write`, otherwise one.
    copies: Vec<RawBuffer>,
    size: u64,
    usage: Usage,
    states: States,
    bound: Option<u32>,
}

pub(crate) struct Texture {
    image: vk::Image,
    /// Null for swapchain images (owned by the swapchain).
    mem: vk::DeviceMemory,
    bytes: u64,
    /// Full view of the natural type (sampling).
    view: vk::ImageView,
    /// Mip 0 storage view (shader_write 2D textures), else null.
    storage_view: vk::ImageView,
    /// One 2D view per (layer, mip) for render targets and depth: index `layer * mips + mip`.
    att_views: Vec<vk::ImageView>,
    kind: TextureKind,
    format: Format,
    width: u32,
    height: u32,
    depth_or_layers: u32,
    mips: u32,
    usage: Usage,
    states: States,
    bound: Option<u32>,
    /// A swapchain image acquired for this frame.
    surface: bool,
    sampled_layout: vk::ImageLayout,
}

impl Texture {
    fn layers(&self) -> u32 {
        match self.kind {
            TextureKind::D2Array | TextureKind::Cube => self.depth_or_layers,
            _ => 1,
        }
    }
    fn depth(&self) -> u32 {
        if self.kind == TextureKind::D3 { self.depth_or_layers } else { 1 }
    }
    fn res(&self) -> conv::Res {
        conv::Res::Texture { sampled_layout: self.sampled_layout, surface: self.surface }
    }
    fn aspect(&self) -> vk::ImageAspectFlags {
        conv::aspect(self.format)
    }
}

pub(crate) struct Sampler {
    raw: vk::Sampler,
    bound: Option<u32>,
}

struct Shader {
    module: vk::ShaderModule,
    stage: ShaderStage,
    entry: CString,
    /// `LocalSize` compiled into a compute shader, when it is not specialisable.
    local_size: Option<[u32; 3]>,
}

pub(crate) struct RenderPipe {
    raw: vk::Pipeline,
    colors: Vec<Format>,
    depth: Option<(Format, bool)>,
}

pub(crate) struct ComputePipe {
    raw: vk::Pipeline,
}

struct Surface {
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    /// Signalled by the frame's submit, waited by present; one per swapchain image.
    present_sems: Vec<vk::Semaphore>,
    /// Signalled by acquire, waited by the frame's submit; one per frame slot.
    acquire_sems: Vec<vk::Semaphore>,
    config: SurfaceConfig,
    /// Out of date or lost: the next acquire returns `SurfaceLost`. Suboptimal is not lost:
    /// the swapchain still presents, and the app owns the size (`configure_surface`), as on
    /// Metal. MoltenVK reports suboptimal on every present to a layer whose bounds differ from
    /// the configured size (always true for a layer outside a window).
    lost: bool,
    /// A suboptimal result was already logged for this swapchain.
    suboptimal_seen: bool,
    #[cfg(target_os = "macos")]
    _layer: objc2::rc::Retained<objc2_quartz_core::CAMetalLayer>,
}

/// Objects kept alive until the frames that may use them have finished.
enum Deferred {
    Buffer(Buffer),
    Texture(Texture),
    Sampler(Sampler),
    Pipeline(vk::Pipeline),
}

#[derive(Clone)]
pub(crate) struct Scope {
    label: &'static str,
    depth: u32,
    begin: u32,
    end: u32,
}

struct FrameRec {
    index: u64,
    slot: usize,
    scopes: Vec<Scope>,
}

pub(crate) enum Upload {
    Buffer { data: Vec<u8>, dst: BufferId, offset: u64 },
    Texture { data: Vec<u8>, dst: TextureId, sub: Subresource },
}

/// Per frame slot: command recording, timestamp queries, upload staging.
pub(crate) struct FrameSlot {
    pool: vk::CommandPool,
    cb: vk::CommandBuffer,
    queries: vk::QueryPool,
    set: vk::DescriptorSet,
    staging: Option<RawBuffer>,
    /// Buffer address table (binding 4): `buf_n` u64 device addresses, persistently mapped.
    addrs: RawBuffer,
}

impl FrameSlot {
    /// Writes buffer table entry `index` of this slot's address table.
    fn set_addr(&self, index: u32, addr: u64) {
        if (index as u64 + 1) * 8 <= self.addrs.bytes {
            // SAFETY: in-bounds write to the persistent mapping; the entry is not read by a
            // pending frame (fresh slots are unused; freed slots retire only once completed).
            unsafe { (self.addrs.ptr as *mut u64).add(index as usize).write(addr) };
        }
    }
}

struct Tables {
    tex_slots: Slots,
    tex_owner: Vec<Option<TextureId>>,
    /// The slot has a storage-image descriptor to clear on retire.
    tex_storage: Vec<bool>,
    buf_slots: Slots,
    buf_owner: Vec<Option<BufferId>>,
    smp_slots: Slots,
    smp_owner: Vec<Option<SamplerId>>,
}

pub(crate) struct State {
    buffers: Pool<Buffer>,
    textures: Pool<Texture>,
    samplers: Pool<Sampler>,
    shaders: Pool<Shader>,
    rpipes: Pool<RenderPipe>,
    cpipes: Pool<ComputePipe>,
    surfaces: Pool<Surface>,
    tables: Tables,
    frames_begun: u64,
    /// Frames submitted (frame f signals the timeline with f + 1).
    submitted: u64,
    completed: u64,
    frame_open: bool,
    in_flight: VecDeque<FrameRec>,
    deferred: Vec<(u64, Deferred)>,
    uploads: Vec<Upload>,
    /// Surfaces acquired in the open frame: (surface, image index, texture).
    acquired: Vec<(SurfaceId, u32, TextureId)>,
    slots: Vec<FrameSlot>,
    timings: Vec<Timing>,
}

/// Validation messages collected by the debug messenger.
struct Messages {
    errors: Mutex<Vec<String>>,
    waived: Mutex<Vec<&'static str>>,
    /// Set once the device turns out to be MoltenVK; [`WAIVED`] applies only then.
    moltenvk: std::sync::atomic::AtomicBool,
    /// Pipelines being created right now whose colour targets include a format that cannot
    /// blend, with blending off ([`RenderPipelineDesc::validate`] rejects blending there).
    /// MoltenVK 1.4.2 warns "Blending is enabled for attachment with format ..." for every
    /// such attachment whatever `blendEnable` says (its check omits `pCA->blendEnable`; fixed
    /// upstream), so that warning is waived only while this is non-zero.
    unblendable_opaque: std::sync::atomic::AtomicU32,
}

/// MoltenVK's false-positive blend warning (see `Messages::unblendable_opaque`).
const MVK_BLEND_WARNING: &str = "Blending is enabled for attachment with format";
const MVK_BLEND_WAIVER: &str = "MoltenVK 1.4.2 reports this for any colour attachment whose format cannot blend, even with \
     blendEnable = VK_FALSE (its check omits pCA->blendEnable); ed_gpu rejects real blending on such formats";

struct Locked(Mutex<State>);
// SAFETY: `State` holds Vulkan handles (plain integers) and raw pointers into persistently
// mapped memory owned by those handles. All access goes through the mutex; Vulkan objects
// that need external synchronisation (descriptor sets on update, pools) are only touched with
// the lock held or by the single encoder that owns the frame slot.
unsafe impl Send for Locked {}
// SAFETY: as above.
unsafe impl Sync for Locked {}

struct Exts {
    debug_utils: Option<ash::ext::debug_utils::Device>,
    swapchain: ash::khr::swapchain::Device,
    surface: ash::khr::surface::Instance,
    #[cfg(target_os = "macos")]
    metal_surface: Option<ash::ext::metal_surface::Instance>,
    colorspace: bool,
}

pub(crate) struct VulkanDevice {
    caps: Caps,
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_instance: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    messages: Box<Messages>,
    pdev: vk::PhysicalDevice,
    raw: ash::Device,
    queue: Mutex<vk::Queue>,
    queue_family: u32,
    exts: Exts,
    mem_props: vk::PhysicalDeviceMemoryProperties,
    timestamp_period: f64,
    timeline: vk::Semaphore,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    desc_pool: vk::DescriptorPool,
    dummy_image: (vk::Image, vk::DeviceMemory, vk::ImageView),
    dummy_buffer: RawBuffer,
    dummy_sampler: vk::Sampler,
    /// Device features this backend enabled that commands depend on.
    indirect_count: bool,
    multi_draw: bool,
    /// `depthClamp` (DepthDesc::clamp); without it such pipelines clip instead.
    depth_clamp: bool,
    validation: bool,
    /// `ED_GPU_VK_VALIDATION=require`: a validation error at submit panics (tests, CI).
    require_validation: bool,
    frames: u32,
    allocated: AtomicU64,
    state: Locked,
}

fn invalid(what: impl Into<String>) -> Error {
    Error::Invalid(what.into())
}

fn vkr(r: vk::Result) -> Error {
    conv::result(r)
}

unsafe extern "system" fn on_message(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    user: *mut c_void,
) -> vk::Bool32 {
    if data.is_null() || user.is_null() {
        return vk::FALSE;
    }
    // SAFETY: the loader passes a valid callback data struct for the duration of the call;
    // `user` is the `Messages` box owned by the device, which outlives the messenger.
    let (data, msgs) = unsafe { (&*data, &*(user as *const Messages)) };
    let cstr = |p: *const std::ffi::c_char| {
        // SAFETY: non-null strings in the callback data are NUL-terminated and live for the call.
        if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
    };
    let id = cstr(data.p_message_id_name);
    let text = cstr(data.p_message);
    let molten = msgs.moltenvk.load(std::sync::atomic::Ordering::Relaxed);
    if molten && text.contains(MVK_BLEND_WARNING) && msgs.unblendable_opaque.load(std::sync::atomic::Ordering::Relaxed) > 0 {
        // Silent: the pipeline provably has blending off on that attachment.
        let mut w = msgs.waived.lock().unwrap_or_else(|e| e.into_inner());
        if !w.contains(&MVK_BLEND_WAIVER) {
            w.push(MVK_BLEND_WAIVER);
        }
        return vk::FALSE;
    }
    if let Some((name, why)) = WAIVED.iter().find(|(n, _)| molten && *n == id) {
        let mut w = msgs.waived.lock().unwrap_or_else(|e| e.into_inner());
        if !w.contains(name) {
            w.push(name);
            eprintln!("[ed_gpu vulkan validation, waived: {why}] {text}");
        }
        return vk::FALSE;
    }
    let sev = if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) { "error" } else { "warning" };
    eprintln!("[ed_gpu vulkan validation {sev}] {text}");
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        msgs.errors.lock().unwrap_or_else(|e| e.into_inner()).push(text);
    }
    vk::FALSE
}

fn load_entry() -> Result<ash::Entry> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("ED_GPU_VULKAN_LIB") {
        candidates.push(p);
    }
    if cfg!(target_os = "macos") {
        candidates.extend(
            ["libvulkan.dylib", "libvulkan.1.dylib", "/opt/homebrew/lib/libvulkan.1.dylib", "/usr/local/lib/libvulkan.1.dylib"].map(String::from),
        );
    } else if cfg!(windows) {
        candidates.push("vulkan-1.dll".into());
    } else {
        candidates.extend(["libvulkan.so.1", "libvulkan.so"].map(String::from));
    }
    let mut errs = Vec::new();
    for c in &candidates {
        // SAFETY: loading the Vulkan loader library runs its initialisers; it is the library
        // this backend is written against.
        match unsafe { ash::Entry::load_from(c) } {
            Ok(e) => return Ok(e),
            Err(e) => errs.push(format!("{c}: {e}")),
        }
    }
    Err(Error::Unsupported(format!("no Vulkan loader found (set ED_GPU_VULKAN_LIB): {}", errs.join("; "))))
}

fn has_ext(list: &[vk::ExtensionProperties], name: &CStr) -> bool {
    list.iter().any(|e| e.extension_name_as_c_str() == Ok(name))
}

impl VulkanDevice {
    pub fn new(desc: &DeviceDesc) -> Result<Self> {
        let entry = load_entry()?;
        let messages = Box::new(Messages { errors: Mutex::new(Vec::new()), waived: Mutex::new(Vec::new()), moltenvk: Default::default(), unblendable_opaque: Default::default() });

        // ---- Instance ----
        // SAFETY: plain queries on a loaded entry.
        let inst_exts = unsafe { entry.enumerate_instance_extension_properties(None) }.map_err(vkr)?;
        // SAFETY: as above.
        let layers = unsafe { entry.enumerate_instance_layer_properties() }.map_err(vkr)?;
        let val_layer = c"VK_LAYER_KHRONOS_validation";
        let require = std::env::var("ED_GPU_VK_VALIDATION").as_deref() == Ok("require");
        let mut use_layer = desc.validation && layers.iter().any(|l| l.layer_name_as_c_str() == Ok(val_layer));
        if desc.validation && !use_layer {
            if require {
                return Err(Error::Unsupported("ED_GPU_VK_VALIDATION=require but VK_LAYER_KHRONOS_validation is not installed".into()));
            }
            eprintln!("[ed_gpu vulkan] validation requested but VK_LAYER_KHRONOS_validation is not installed; running without it");
        }
        let portability = has_ext(&inst_exts, ash::khr::portability_enumeration::NAME);
        #[cfg(target_os = "macos")]
        let metal_surface_ext = has_ext(&inst_exts, ash::ext::metal_surface::NAME);
        let debug_utils = has_ext(&inst_exts, ash::ext::debug_utils::NAME);
        let colorspace = has_ext(&inst_exts, ash::ext::swapchain_colorspace::NAME);
        let user = &*messages as *const Messages as *mut c_void;
        let (instance, use_layer) = loop {
            let layer_exts = if use_layer {
                // SAFETY: plain query for a layer that exists.
                unsafe { entry.enumerate_instance_extension_properties(Some(val_layer)) }.unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut ext_names: Vec<&CStr> = vec![ash::khr::surface::NAME];
            if portability {
                ext_names.push(ash::khr::portability_enumeration::NAME);
            }
            #[cfg(target_os = "macos")]
            if metal_surface_ext {
                ext_names.push(ash::ext::metal_surface::NAME);
            }
            if debug_utils {
                ext_names.push(ash::ext::debug_utils::NAME);
            }
            if colorspace {
                ext_names.push(ash::ext::swapchain_colorspace::NAME);
            }
            let validation_features = use_layer && has_ext(&layer_exts, ash::ext::validation_features::NAME);
            if validation_features {
                ext_names.push(ash::ext::validation_features::NAME);
            }
            if !has_ext(&inst_exts, ash::khr::surface::NAME) {
                ext_names.retain(|n| *n != ash::khr::surface::NAME);
            }
            let ext_ptrs: Vec<_> = ext_names.iter().map(|n| n.as_ptr()).collect();
            let layer_ptrs: Vec<_> = if use_layer { vec![val_layer.as_ptr()] } else { Vec::new() };
            let app = vk::ApplicationInfo::default().application_name(c"ed_gpu").engine_name(c"ed_gpu").api_version(vk::API_VERSION_1_3);
            let mut messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
                .message_type(vk::DebugUtilsMessageTypeFlagsEXT::GENERAL | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE)
                .pfn_user_callback(Some(on_message))
                .user_data(user);
            let syncval = std::env::var("ED_GPU_VK_SYNCVAL").map(|v| v != "0").unwrap_or(true);
            let enables: Vec<vk::ValidationFeatureEnableEXT> = if syncval { vec![vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION] } else { Vec::new() };
            let mut vfeatures = vk::ValidationFeaturesEXT::default().enabled_validation_features(&enables);
            let mut ici = vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_extension_names(&ext_ptrs)
                .enabled_layer_names(&layer_ptrs)
                .flags(if portability { vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR } else { vk::InstanceCreateFlags::empty() });
            if debug_utils && use_layer {
                ici = ici.push_next(&mut messenger_info);
            }
            if validation_features {
                ici = ici.push_next(&mut vfeatures);
            }
            // SAFETY: every pointer in the create info is alive for the call.
            match unsafe { entry.create_instance(&ici, None) } {
                Ok(i) => break (i, use_layer),
                // The manifest is installed but the library does not load (Homebrew's manifest
                // names a bare dylib, found only through DYLD_FALLBACK_LIBRARY_PATH).
                Err(vk::Result::ERROR_LAYER_NOT_PRESENT) if use_layer && !require => {
                    eprintln!(
                        "[ed_gpu vulkan] VK_LAYER_KHRONOS_validation is listed but failed to load; running WITHOUT validation \
                         (Homebrew: set DYLD_FALLBACK_LIBRARY_PATH=/opt/homebrew/lib; ED_GPU_VK_VALIDATION=require makes this an error)"
                    );
                    messages.errors.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    use_layer = false;
                }
                Err(e) => return Err(Error::Unsupported(format!("vkCreateInstance: {e:?}"))),
            }
        };
        let debug_instance = if debug_utils && use_layer {
            let di = ash::ext::debug_utils::Instance::new(&entry, &instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
                .message_type(vk::DebugUtilsMessageTypeFlagsEXT::GENERAL | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE)
                .pfn_user_callback(Some(on_message))
                .user_data(user);
            // SAFETY: valid instance; the user data box outlives the messenger (destroyed in Drop).
            let m = unsafe { di.create_debug_utils_messenger(&info, None) }.map_err(vkr)?;
            Some((di, m))
        } else {
            None
        };

        // ---- Physical device and queue ----
        // SAFETY: valid instance.
        let pdevs = unsafe { instance.enumerate_physical_devices() }.map_err(vkr)?;
        let pick = pdevs
            .iter()
            .copied()
            .filter_map(|pd| {
                // SAFETY: valid physical device from the instance.
                let props = unsafe { instance.get_physical_device_properties(pd) };
                if props.api_version < vk::API_VERSION_1_3 {
                    return None;
                }
                // SAFETY: as above.
                let fams = unsafe { instance.get_physical_device_queue_family_properties(pd) };
                let qf = fams.iter().position(|f| f.queue_flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE))? as u32;
                let rank = match props.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                    _ => 2,
                };
                Some((rank, pd, qf, props, fams[qf as usize].timestamp_valid_bits))
            })
            .min_by_key(|x| x.0);
        let Some((_, pdev, queue_family, props, ts_bits)) = pick else {
            return Err(Error::Unsupported("no Vulkan 1.3 device with a graphics+compute queue".into()));
        };

        // SAFETY: valid physical device.
        let dev_exts = unsafe { instance.enumerate_device_extension_properties(pdev) }.map_err(vkr)?;
        let mut s12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut s13 = vk::PhysicalDeviceVulkan13Features::default();
        let s10 = {
            let mut f2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut s12).push_next(&mut s13);
            // SAFETY: the chain points at live structs.
            unsafe { instance.get_physical_device_features2(pdev, &mut f2) };
            f2.features
        };
        let t = |b: vk::Bool32| b == vk::TRUE;
        let required = [
            ("dynamicRendering", t(s13.dynamic_rendering)),
            ("synchronization2", t(s13.synchronization2)),
            ("timelineSemaphore", t(s12.timeline_semaphore)),
            ("runtimeDescriptorArray", t(s12.runtime_descriptor_array)),
            ("descriptorBindingPartiallyBound", t(s12.descriptor_binding_partially_bound)),
            ("descriptorBindingSampledImageUpdateAfterBind", t(s12.descriptor_binding_sampled_image_update_after_bind)),
            ("descriptorBindingStorageImageUpdateAfterBind", t(s12.descriptor_binding_storage_image_update_after_bind)),
            ("descriptorBindingStorageBufferUpdateAfterBind", t(s12.descriptor_binding_storage_buffer_update_after_bind)),
            ("descriptorBindingUpdateUnusedWhilePending", t(s12.descriptor_binding_update_unused_while_pending)),
            ("shaderSampledImageArrayNonUniformIndexing", t(s12.shader_sampled_image_array_non_uniform_indexing)),
            ("shaderStorageBufferArrayNonUniformIndexing", t(s12.shader_storage_buffer_array_non_uniform_indexing)),
            ("shaderStorageImageArrayNonUniformIndexing", t(s12.shader_storage_image_array_non_uniform_indexing)),
            ("bufferDeviceAddress", t(s12.buffer_device_address)),
            ("shaderInt64", t(s10.shader_int64)),
        ];
        let missing: Vec<_> = required.iter().filter(|r| !r.1).map(|r| r.0).collect();
        if !missing.is_empty() {
            return Err(Error::Unsupported(format!("Vulkan device lacks {}", missing.join(", "))));
        }
        let e10 = vk::PhysicalDeviceFeatures::default()
            .multi_draw_indirect(t(s10.multi_draw_indirect))
            .depth_clamp(t(s10.depth_clamp))
            .draw_indirect_first_instance(t(s10.draw_indirect_first_instance))
            .sampler_anisotropy(t(s10.sampler_anisotropy))
            .texture_compression_bc(t(s10.texture_compression_bc))
            .texture_compression_astc_ldr(t(s10.texture_compression_astc_ldr))
            .shader_int64(t(s10.shader_int64))
            // Fragment `SV_PrimitiveID` (the visibility buffer) needs one of these; Slang
            // fragment modules declare `Tessellation` for it (`slang::fragment_primitive_id_without_geometry`)
            // because MoltenVK has no geometry shaders.
            .tessellation_shader(t(s10.tessellation_shader))
            .geometry_shader(t(s10.geometry_shader))
            .fragment_stores_and_atomics(t(s10.fragment_stores_and_atomics))
            // The buffer table is read-write in every stage (as on Metal), so vertex shaders
            // that read it need this unless every buffer alias is declared `readonly`.
            .vertex_pipeline_stores_and_atomics(t(s10.vertex_pipeline_stores_and_atomics))
            .shader_storage_image_write_without_format(t(s10.shader_storage_image_write_without_format))
            .shader_storage_image_read_without_format(t(s10.shader_storage_image_read_without_format))
            .sparse_binding(false);
        let mut e12 = vk::PhysicalDeviceVulkan12Features::default()
            .timeline_semaphore(true)
            .descriptor_indexing(t(s12.descriptor_indexing))
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_sampled_image_update_after_bind(true)
            .descriptor_binding_storage_image_update_after_bind(true)
            .descriptor_binding_storage_buffer_update_after_bind(true)
            .descriptor_binding_update_unused_while_pending(true)
            .shader_sampled_image_array_non_uniform_indexing(true)
            .shader_storage_buffer_array_non_uniform_indexing(true)
            .shader_storage_image_array_non_uniform_indexing(true)
            .draw_indirect_count(t(s12.draw_indirect_count))
            .host_query_reset(t(s12.host_query_reset))
            .scalar_block_layout(t(s12.scalar_block_layout))
            .shader_buffer_int64_atomics(t(s12.shader_buffer_int64_atomics))
            .buffer_device_address(true);
        // maintenance4: glslang targeting Vulkan 1.3 writes `local_size_x_id` as LocalSizeId.
        let mut e13 = vk::PhysicalDeviceVulkan13Features::default().dynamic_rendering(true).synchronization2(true).maintenance4(t(s13.maintenance4));

        let mut dext: Vec<&CStr> = vec![];
        let swapchain_ext = has_ext(&dev_exts, ash::khr::swapchain::NAME);
        if swapchain_ext {
            dext.push(ash::khr::swapchain::NAME);
        }
        if has_ext(&dev_exts, ash::khr::portability_subset::NAME) {
            dext.push(ash::khr::portability_subset::NAME);
        }
        let budget_ext = has_ext(&dev_exts, ash::ext::memory_budget::NAME);
        if budget_ext {
            dext.push(ash::ext::memory_budget::NAME);
        }
        let dext_ptrs: Vec<_> = dext.iter().map(|n| n.as_ptr()).collect();
        let prio = [1.0f32];
        let qci = [vk::DeviceQueueCreateInfo::default().queue_family_index(queue_family).queue_priorities(&prio)];
        let dci = vk::DeviceCreateInfo::default()
            .queue_create_infos(&qci)
            .enabled_extension_names(&dext_ptrs)
            .enabled_features(&e10)
            .push_next(&mut e12)
            .push_next(&mut e13);
        // SAFETY: every pointer in the create info is alive for the call.
        let raw = unsafe { instance.create_device(pdev, &dci, None) }.map_err(|e| Error::Unsupported(format!("vkCreateDevice: {e:?}")))?;
        // SAFETY: family and index were requested in the create info.
        let queue = unsafe { raw.get_device_queue(queue_family, 0) };

        // ---- Properties for caps ----
        let mut p12 = vk::PhysicalDeviceVulkan12Properties::default();
        {
            let mut p2 = vk::PhysicalDeviceProperties2::default().push_next(&mut p12);
            // SAFETY: chain points at live structs.
            unsafe { instance.get_physical_device_properties2(pdev, &mut p2) };
        }
        // SAFETY: valid physical device.
        let mem_props = unsafe { instance.get_physical_device_memory_properties(pdev) };
        let lim = props.limits;
        let moltenvk = p12.driver_id == vk::DriverId::MOLTENVK;
        messages.moltenvk.store(moltenvk, std::sync::atomic::Ordering::Relaxed);
        let mut tex_n = TEXTURE_TABLE
            .min(p12.max_descriptor_set_update_after_bind_sampled_images)
            .min(p12.max_descriptor_set_update_after_bind_storage_images)
            .min(p12.max_per_stage_descriptor_update_after_bind_sampled_images)
            .min(p12.max_per_stage_descriptor_update_after_bind_storage_images);
        let buf_n = BUFFER_TABLE.min(p12.max_descriptor_set_update_after_bind_storage_buffers).min(p12.max_per_stage_descriptor_update_after_bind_storage_buffers);
        let smp_n = SAMPLER_TABLE.min(p12.max_descriptor_set_update_after_bind_samplers).min(p12.max_per_stage_descriptor_update_after_bind_samplers);
        let total = p12.max_per_stage_update_after_bind_resources;
        if 2 * tex_n as u64 + buf_n as u64 + smp_n as u64 > total as u64 {
            tex_n = (total.saturating_sub(buf_n + smp_n)) / 2;
        }
        // The per-binding limits are not the whole story: the set as a whole must also pass
        // vkGetDescriptorSetLayoutSupport (MoltenVK reports a small maxPerSetDescriptors and
        // checks the sum). Halve the tables until the driver accepts the layout.
        let stages = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::COMPUTE;
        let uab = vk::DescriptorBindingFlags::UPDATE_AFTER_BIND | vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING;
        let bflags = [uab, uab, uab, uab, vk::DescriptorBindingFlags::UPDATE_AFTER_BIND];
        let table_bindings = |tex_n: u32, buf_n: u32, smp_n: u32| {
            [
                (B_TEXTURES, vk::DescriptorType::SAMPLED_IMAGE, tex_n),
                (B_IMAGES, vk::DescriptorType::STORAGE_IMAGE, tex_n),
                (B_BUFFERS, vk::DescriptorType::STORAGE_BUFFER, buf_n),
                (B_SAMPLERS, vk::DescriptorType::SAMPLER, smp_n),
                (B_ADDRS, vk::DescriptorType::STORAGE_BUFFER, 1),
            ]
            .map(|(b, ty, n)| vk::DescriptorSetLayoutBinding::default().binding(b).descriptor_type(ty).descriptor_count(n).stage_flags(stages))
        };
        let (mut buf_n, mut smp_n) = (buf_n, smp_n);
        loop {
            if tex_n < MIN_TABLE || buf_n < MIN_TABLE || smp_n < 16 {
                return Err(Error::Unsupported(format!("update-after-bind descriptor limits too small for bindless tables ({tex_n}/{buf_n}/{smp_n})")));
            }
            let bindings = table_bindings(tex_n, buf_n, smp_n);
            let mut flags_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&bflags);
            let sli = vk::DescriptorSetLayoutCreateInfo::default()
                .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                .bindings(&bindings)
                .push_next(&mut flags_info);
            let mut support = vk::DescriptorSetLayoutSupport::default();
            // SAFETY: valid device; pointers alive for the call.
            unsafe { raw.get_descriptor_set_layout_support(&sli, &mut support) };
            if support.supported == vk::TRUE || moltenvk {
                break;
            }
            tex_n /= 2;
            buf_n /= 2;
            smp_n = (smp_n / 2).max(16);
        }

        let budget = {
            let heaps = &mem_props.memory_heaps[..mem_props.memory_heap_count as usize];
            let local = |i: usize| heaps[i].flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL);
            if budget_ext {
                let mut b = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
                let mut m2 = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut b);
                // SAFETY: chain points at live structs.
                unsafe { instance.get_physical_device_memory_properties2(pdev, &mut m2) };
                (0..heaps.len()).filter(|&i| local(i)).map(|i| b.heap_budget[i]).sum::<u64>()
            } else {
                (0..heaps.len()).filter(|&i| local(i)).map(|i| heaps[i].size).max().unwrap_or(0) / 10 * 8
            }
        };
        let name = props.device_name_as_c_str().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
        let timers = if ts_bits > 0 && lim.timestamp_compute_and_graphics == vk::TRUE { TimerSource::PassBoundary } else { TimerSource::None };
        let caps = Caps {
            backend: Backend::Vulkan,
            device_name: format!("{name} (Vulkan {}.{})", vk::api_version_major(props.api_version), vk::api_version_minor(props.api_version)),
            memory_budget: budget,
            unified_memory: props.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU,
            bindless_tier: 2,
            max_bindless_textures: tex_n,
            max_bindless_buffers: buf_n,
            max_bindless_samplers: smp_n,
            max_texture_size: lim.max_image_dimension2_d,
            max_workgroup_threads: lim.max_compute_work_group_invocations,
            max_workgroup_memory: lim.max_compute_shared_memory_size,
            indirect_count: t(s12.draw_indirect_count),
            native_multi_draw_indirect: t(s10.multi_draw_indirect),
            mesh_shaders: has_ext(&dev_exts, ash::ext::mesh_shader::NAME),
            ray_tracing: has_ext(&dev_exts, ash::khr::ray_query::NAME) && has_ext(&dev_exts, ash::khr::acceleration_structure::NAME),
            atomics_64: t(s12.shader_buffer_int64_atomics),
            sparse_textures: t(s10.sparse_residency_image2_d),
            bc_textures: t(s10.texture_compression_bc),
            astc_textures: t(s10.texture_compression_astc_ldr),
            timers,
            hdr_output: colorspace,
        };

        // ---- Bindless descriptor sets ----
        let bindings = table_bindings(tex_n, buf_n, smp_n);
        let mut flags_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&bflags);
        let sli = vk::DescriptorSetLayoutCreateInfo::default()
            .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
            .bindings(&bindings)
            .push_next(&mut flags_info);
        // SAFETY: valid device; pointers alive for the call.
        let set_layout = unsafe { raw.create_descriptor_set_layout(&sli, None) }.map_err(vkr)?;
        let push = [vk::PushConstantRange::default().stage_flags(stages).offset(0).size(DRAW_CONSTANTS_BYTES as u32)];
        let set_layouts = [set_layout];
        // SAFETY: as above.
        let layout = unsafe { raw.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push), None) }.map_err(vkr)?;
        let frames = desc.frames_in_flight;
        let sizes = [
            (vk::DescriptorType::SAMPLED_IMAGE, tex_n),
            (vk::DescriptorType::STORAGE_IMAGE, tex_n),
            (vk::DescriptorType::STORAGE_BUFFER, buf_n + 1),
            (vk::DescriptorType::SAMPLER, smp_n),
        ]
        .map(|(ty, n)| vk::DescriptorPoolSize::default().ty(ty).descriptor_count(n * frames));
        // SAFETY: as above.
        let desc_pool = unsafe {
            raw.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND).max_sets(frames).pool_sizes(&sizes), None)
        }
        .map_err(vkr)?;
        let layouts: Vec<_> = (0..frames).map(|_| set_layout).collect();
        // SAFETY: pool and layouts are valid.
        let sets = unsafe { raw.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(desc_pool).set_layouts(&layouts)) }.map_err(vkr)?;

        // ---- Timeline ----
        let mut tl = vk::SemaphoreTypeCreateInfo::default().semaphore_type(vk::SemaphoreType::TIMELINE).initial_value(0);
        // SAFETY: valid device.
        let timeline = unsafe { raw.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut tl), None) }.map_err(vkr)?;

        let exts = Exts {
            debug_utils: if debug_utils { Some(ash::ext::debug_utils::Device::new(&instance, &raw)) } else { None },
            swapchain: ash::khr::swapchain::Device::new(&instance, &raw),
            surface: ash::khr::surface::Instance::new(&entry, &instance),
            #[cfg(target_os = "macos")]
            metal_surface: if metal_surface_ext { Some(ash::ext::metal_surface::Instance::new(&entry, &instance)) } else { None },
            colorspace,
        };
        let _ = swapchain_ext;

        let mut dev = Self {
            caps,
            _entry: entry,
            instance,
            debug_instance,
            messages,
            pdev,
            raw,
            queue: Mutex::new(queue),
            queue_family,
            exts,
            mem_props,
            timestamp_period: lim.timestamp_period as f64,
            timeline,
            set_layout,
            layout,
            desc_pool,
            dummy_image: (vk::Image::null(), vk::DeviceMemory::null(), vk::ImageView::null()),
            dummy_buffer: RawBuffer { raw: vk::Buffer::null(), mem: vk::DeviceMemory::null(), ptr: std::ptr::null_mut(), bytes: 0 },
            dummy_sampler: vk::Sampler::null(),
            indirect_count: t(s12.draw_indirect_count),
            multi_draw: t(s10.multi_draw_indirect),
            depth_clamp: t(s10.depth_clamp),
            validation: desc.validation,
            require_validation: require && use_layer,
            frames,
            allocated: AtomicU64::new(0),
            state: Locked(Mutex::new(State {
                buffers: Pool::default(),
                textures: Pool::default(),
                samplers: Pool::default(),
                shaders: Pool::default(),
                rpipes: Pool::default(),
                cpipes: Pool::default(),
                surfaces: Pool::default(),
                tables: Tables {
                    tex_slots: Slots::new(tex_n),
                    tex_owner: Vec::new(),
                    tex_storage: Vec::new(),
                    buf_slots: Slots::new(buf_n),
                    buf_owner: Vec::new(),
                    smp_slots: Slots::new(smp_n),
                    smp_owner: Vec::new(),
                },
                frames_begun: 0,
                submitted: 0,
                completed: 0,
                frame_open: false,
                in_flight: VecDeque::new(),
                deferred: Vec::new(),
                uploads: Vec::new(),
                acquired: Vec::new(),
                slots: Vec::new(),
                timings: Vec::new(),
            })),
        };
        // From here on Drop cleans up whatever was created.
        dev.init_slots(&sets)?;
        dev.init_dummies()?;
        Ok(dev)
    }

    fn init_slots(&mut self, sets: &[vk::DescriptorSet]) -> Result<()> {
        let mut slots = Vec::new();
        for (f, &set) in sets.iter().enumerate() {
            // SAFETY: valid device; family exists.
            let pool = unsafe {
                self.raw.create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(self.queue_family).flags(vk::CommandPoolCreateFlags::TRANSIENT), None)
            }
            .map_err(vkr)?;
            // SAFETY: valid pool.
            let cb = unsafe { self.raw.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1)) }
                .map_err(vkr)?[0];
            // SAFETY: valid device.
            let queries = unsafe { self.raw.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(MAX_QUERIES), None) }.map_err(vkr)?;
            self.name(set, &format!("ed_gpu bindless set (frame slot {f})"));
            let n = self.caps.max_bindless_buffers as u64;
            let addrs = self.make_buffer(n * 8, vk::BufferUsageFlags::STORAGE_BUFFER, MemKind::Upload, &format!("ed_gpu buffer address table (frame slot {f})"))?;
            // SAFETY: fresh host-visible mapping of at least n * 8 bytes.
            unsafe { std::ptr::write_bytes(addrs.ptr, 0, (n * 8) as usize) };
            let info = vk::DescriptorBufferInfo::default().buffer(addrs.raw).range(vk::WHOLE_SIZE);
            self.write_descriptor(set, B_ADDRS, 0, None, Some(info));
            slots.push(FrameSlot { pool, cb, queries, set, staging: None, addrs });
        }
        self.state.0.get_mut().unwrap_or_else(|e| e.into_inner()).slots = slots;
        Ok(())
    }

    /// Dummy descriptors written over retired table slots, and their one-time layout change.
    fn init_dummies(&mut self) -> Result<()> {
        let ici = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D { width: 1, height: 1, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE);
        // SAFETY: valid device and create info.
        let image = unsafe { self.raw.create_image(&ici, None) }.map_err(vkr)?;
        // SAFETY: valid image.
        let req = unsafe { self.raw.get_image_memory_requirements(image) };
        let (mem, _) = self.alloc(req, MemKind::Gpu, None)?;
        // SAFETY: fresh image and memory of the required size.
        unsafe { self.raw.bind_image_memory(image, mem, 0) }.map_err(vkr)?;
        let view = self.make_view(image, vk::ImageViewType::TYPE_2D, vk::Format::R8G8B8A8_UNORM, vk::ImageAspectFlags::COLOR, 0, 1, 0, 1)?;
        self.dummy_image = (image, mem, view);
        self.dummy_buffer = self.make_buffer(16, vk::BufferUsageFlags::STORAGE_BUFFER, MemKind::Gpu, "ed_gpu dummy")?;
        // SAFETY: valid device.
        self.dummy_sampler = unsafe { self.raw.create_sampler(&vk::SamplerCreateInfo::default(), None) }.map_err(vkr)?;
        // One-shot: dummy image to GENERAL.
        let cb = self.state.0.get_mut().unwrap_or_else(|e| e.into_inner()).slots[0].cb;
        // SAFETY: the slot's command buffer is idle (no frame has started).
        unsafe {
            self.raw.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(vkr)?;
            let b = [vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::NONE)
                .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .dst_access_mask(vk::AccessFlags2::SHADER_READ)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::GENERAL)
                .image(image)
                .subresource_range(range(vk::ImageAspectFlags::COLOR, 0, 1, 0, 1))];
            self.raw.cmd_pipeline_barrier2(cb, &vk::DependencyInfo::default().image_memory_barriers(&b));
            self.raw.end_command_buffer(cb).map_err(vkr)?;
            let cbs = [vk::CommandBufferSubmitInfo::default().command_buffer(cb)];
            let q = *self.queue.get_mut().unwrap_or_else(|e| e.into_inner());
            self.raw.queue_submit2(q, &[vk::SubmitInfo2::default().command_buffer_infos(&cbs)], vk::Fence::null()).map_err(vkr)?;
            self.raw.queue_wait_idle(q).map_err(vkr)?;
            let pool = self.state.0.get_mut().unwrap_or_else(|e| e.into_inner()).slots[0].pool;
            self.raw.reset_command_pool(pool, vk::CommandPoolResetFlags::empty()).map_err(vkr)?;
        }
        Ok(())
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Validation errors reported since the last call.
    pub(crate) fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.messages.errors.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn check_validation(&self, what: &str) -> Result<()> {
        let e = self.take_errors();
        if e.is_empty() { Ok(()) } else { Err(invalid(format!("{what}: Vulkan validation: {}", e.join(" | ")))) }
    }

    fn name<H: Handle>(&self, h: H, label: &str) {
        if let Some(du) = &self.exts.debug_utils
            && let Ok(c) = CString::new(label)
        {
            // SAFETY: valid handle owned by this device; the name is copied.
            let _ = unsafe { du.set_debug_utils_object_name(&vk::DebugUtilsObjectNameInfoEXT::default().object_handle(h).object_name(&c)) };
        }
    }

    fn memory_type(&self, bits: u32, kind: MemKind) -> Option<u32> {
        use vk::MemoryPropertyFlags as M;
        let (req, pref) = match kind {
            MemKind::Gpu => (M::DEVICE_LOCAL, M::empty()),
            MemKind::Upload => (M::HOST_VISIBLE | M::HOST_COHERENT, M::empty()),
            MemKind::Readback => (M::HOST_VISIBLE | M::HOST_COHERENT, M::HOST_CACHED),
        };
        let types = &self.mem_props.memory_types[..self.mem_props.memory_type_count as usize];
        let find = |want: M| (0..types.len() as u32).find(|&i| bits & (1 << i) != 0 && types[i as usize].property_flags.contains(want));
        find(req | pref).or_else(|| find(req)).or_else(|| if kind == MemKind::Gpu { find(M::empty()) } else { None })
    }

    /// Allocates and (for host kinds) maps memory. Returns the memory and the mapping.
    fn alloc(&self, req: vk::MemoryRequirements, kind: MemKind, label: Option<&str>) -> Result<(vk::DeviceMemory, *mut u8)> {
        self.alloc_flags(req, kind, label, false)
    }

    /// As [`Self::alloc`]; `device_address` allocates with `VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT`.
    fn alloc_flags(&self, req: vk::MemoryRequirements, kind: MemKind, label: Option<&str>, device_address: bool) -> Result<(vk::DeviceMemory, *mut u8)> {
        let ty = self.memory_type(req.memory_type_bits, kind).ok_or_else(|| Error::Backend(format!("no memory type for {:?}", label)))?;
        let mut flags = vk::MemoryAllocateFlagsInfo::default().flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);
        let mut info = vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ty);
        if device_address {
            info = info.push_next(&mut flags);
        }
        // SAFETY: valid device; size and type from the resource's requirements.
        let mem = unsafe { self.raw.allocate_memory(&info, None) }.map_err(vkr)?;
        self.allocated.fetch_add(req.size, Ordering::Relaxed);
        let ptr = if kind == MemKind::Gpu {
            std::ptr::null_mut()
        } else {
            // SAFETY: host-visible memory just allocated, mapped once for its lifetime.
            match unsafe { self.raw.map_memory(mem, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty()) } {
                Ok(p) => p as *mut u8,
                Err(e) => {
                    self.free_mem(mem, req.size);
                    return Err(vkr(e));
                }
            }
        };
        if let Some(l) = label {
            self.name(mem, l);
        }
        Ok((mem, ptr))
    }

    fn free_mem(&self, mem: vk::DeviceMemory, size: u64) {
        if mem != vk::DeviceMemory::null() {
            // SAFETY: memory owned by this device; its resources are destroyed or unused.
            unsafe { self.raw.free_memory(mem, None) };
            self.allocated.fetch_sub(size, Ordering::Relaxed);
        }
    }

    fn make_buffer(&self, size: u64, usage: vk::BufferUsageFlags, kind: MemKind, label: &str) -> Result<RawBuffer> {
        // Storage buffers are reachable by device address (Slang ABI, binding 4).
        let addressable = usage.contains(vk::BufferUsageFlags::STORAGE_BUFFER);
        let usage = if addressable { usage | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS } else { usage };
        // SAFETY: valid device and create info.
        let raw = unsafe { self.raw.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE), None) }.map_err(vkr)?;
        // SAFETY: valid buffer.
        let req = unsafe { self.raw.get_buffer_memory_requirements(raw) };
        let (mem, ptr) = match self.alloc_flags(req, kind, Some(label), addressable) {
            Ok(m) => m,
            Err(e) => {
                // SAFETY: unused buffer.
                unsafe { self.raw.destroy_buffer(raw, None) };
                return Err(e);
            }
        };
        // SAFETY: fresh buffer and memory of the required size.
        if let Err(e) = unsafe { self.raw.bind_buffer_memory(raw, mem, 0) } {
            // SAFETY: unused buffer.
            unsafe { self.raw.destroy_buffer(raw, None) };
            self.free_mem(mem, req.size);
            return Err(vkr(e));
        }
        self.name(raw, label);
        Ok(RawBuffer { raw, mem, ptr, bytes: req.size })
    }

    fn free_buffer(&self, b: RawBuffer) {
        if b.raw != vk::Buffer::null() {
            // SAFETY: no in-flight frame uses it (deferred until retired).
            unsafe { self.raw.destroy_buffer(b.raw, None) };
        }
        self.free_mem(b.mem, b.bytes);
    }

    #[allow(clippy::too_many_arguments)]
    fn make_view(&self, image: vk::Image, ty: vk::ImageViewType, format: vk::Format, aspect: vk::ImageAspectFlags, mip: u32, mips: u32, layer: u32, layers: u32) -> Result<vk::ImageView> {
        let info = vk::ImageViewCreateInfo::default().image(image).view_type(ty).format(format).subresource_range(range(aspect, mip, mips, layer, layers));
        // SAFETY: valid image; the range is within it (callers pass validated values).
        unsafe { self.raw.create_image_view(&info, None) }.map_err(vkr)
    }

    fn free_texture(&self, t: Texture) {
        // SAFETY: no in-flight frame uses these (deferred until retired). Swapchain images and
        // their views belong to the surface and are not destroyed here.
        unsafe {
            if !t.surface {
                for v in t.att_views.iter().chain([&t.view, &t.storage_view]) {
                    if *v != vk::ImageView::null() {
                        self.raw.destroy_image_view(*v, None);
                    }
                }
                self.raw.destroy_image(t.image, None);
            }
        }
        if !t.surface {
            self.free_mem(t.mem, t.bytes);
        }
    }

    fn free_deferred(&self, d: Deferred) {
        match d {
            Deferred::Buffer(b) => b.copies.into_iter().for_each(|c| self.free_buffer(c)),
            Deferred::Texture(t) => self.free_texture(t),
            // SAFETY: (both) no in-flight frame uses it.
            Deferred::Sampler(s) => unsafe { self.raw.destroy_sampler(s.raw, None) },
            Deferred::Pipeline(p) => unsafe { self.raw.destroy_pipeline(p, None) },
        }
    }

    fn retire_after(st: &State) -> u64 {
        st.frames_begun
    }

    fn defer(&self, st: &mut State, d: Deferred) {
        let after = Self::retire_after(st);
        st.deferred.push((after, d));
    }

    fn timeline_value(&self) -> u64 {
        // SAFETY: valid timeline semaphore.
        unsafe { self.raw.get_semaphore_counter_value(self.timeline) }.unwrap_or(0)
    }

    fn wait_value(&self, v: u64) {
        let sems = [self.timeline];
        let vals = [v];
        // SAFETY: valid semaphore; the value is one a submitted frame signals.
        let _ = unsafe { self.raw.wait_semaphores(&vk::SemaphoreWaitInfo::default().semaphores(&sems).values(&vals), u64::MAX) };
    }

    /// Retire finished frames: timings, bindless slot reuse (dummy descriptors), then deferred
    /// destruction (in that order, so no descriptor outlives its object).
    pub(crate) fn retire(&self, st: &mut State) {
        let done = self.timeline_value();
        while let Some(f) = st.in_flight.front() {
            if f.index + 1 > done {
                break;
            }
            let f = st.in_flight.pop_front().expect("front exists");
            self.resolve_timings(st, &f);
            st.completed = f.index + 1;
        }
        let completed = st.completed;
        let mut freed = Vec::new();
        let mut writes: Vec<(u32, u32, Option<vk::DescriptorImageInfo>, Option<vk::DescriptorBufferInfo>)> = Vec::new();
        let dummy_img = vk::DescriptorImageInfo::default().image_view(self.dummy_image.2).image_layout(vk::ImageLayout::GENERAL);
        st.tables.tex_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            writes.push((B_TEXTURES, s, Some(dummy_img), None));
            if st.tables.tex_storage.get(s as usize).copied().unwrap_or(false) {
                st.tables.tex_storage[s as usize] = false;
                writes.push((B_IMAGES, s, Some(dummy_img), None));
            }
        }
        st.tables.buf_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            writes.push((B_BUFFERS, s, None, Some(vk::DescriptorBufferInfo::default().buffer(self.dummy_buffer.raw).range(vk::WHOLE_SIZE))));
            for slot in &st.slots {
                slot.set_addr(s, 0);
            }
        }
        st.tables.smp_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            writes.push((B_SAMPLERS, s, Some(vk::DescriptorImageInfo::default().sampler(self.dummy_sampler)), None));
        }
        if !writes.is_empty() {
            let sets: Vec<_> = st.slots.iter().map(|s| s.set).collect();
            for set in sets {
                for (b, s, img, bufi) in &writes {
                    self.write_descriptor(set, *b, *s, *img, *bufi);
                }
            }
        }
        let mut i = 0;
        while i < st.deferred.len() {
            if st.deferred[i].0 <= completed {
                let (_, d) = st.deferred.swap_remove(i);
                self.free_deferred(d);
            } else {
                i += 1;
            }
        }
    }

    fn write_descriptor(&self, set: vk::DescriptorSet, binding: u32, index: u32, img: Option<vk::DescriptorImageInfo>, buf: Option<vk::DescriptorBufferInfo>) {
        let ty = match binding {
            B_TEXTURES => vk::DescriptorType::SAMPLED_IMAGE,
            B_IMAGES => vk::DescriptorType::STORAGE_IMAGE,
            B_BUFFERS | B_ADDRS => vk::DescriptorType::STORAGE_BUFFER,
            _ => vk::DescriptorType::SAMPLER,
        };
        let imgs = img.map(|i| [i]);
        let bufs = buf.map(|b| [b]);
        let mut w = vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).dst_array_element(index).descriptor_type(ty);
        if let Some(i) = &imgs {
            w = w.image_info(i);
        }
        if let Some(b) = &bufs {
            w = w.buffer_info(b);
        }
        // SAFETY: called with the state lock held (external sync of the set); the descriptor is
        // not used by any pending frame (update-unused-while-pending).
        unsafe { self.raw.update_descriptor_sets(&[w], &[]) };
    }

    fn resolve_timings(&self, st: &mut State, f: &FrameRec) {
        if f.scopes.is_empty() {
            return;
        }
        let n = f.scopes.iter().map(|s| s.end.max(s.begin) + 1).max().unwrap_or(0).min(MAX_QUERIES);
        let mut stamps = vec![0u64; n as usize];
        // SAFETY: the frame finished and wrote every query below `n` (they are allocated in
        // order), so WAIT returns. Without WAIT, MoltenVK often reports NOT_READY here: it
        // resolves timestamps after the fence signals, and most frames read zeros.
        let ok = unsafe { self.raw.get_query_pool_results(st.slots[f.slot].queries, 0, &mut stamps, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT) }.is_ok();
        st.timings = f
            .scopes
            .iter()
            .map(|s| {
                let (a, b) = (stamps[s.begin as usize], stamps[s.end as usize]);
                let ms = if ok && b > a { ((b - a) as f64 * self.timestamp_period / 1.0e6) as f32 } else { 0.0 };
                Timing { label: s.label, depth: s.depth, gpu_ms: ms }
            })
            .collect();
    }

    fn format_supports(&self, format: vk::Format, need: vk::FormatFeatureFlags) -> bool {
        // SAFETY: plain query.
        let p = unsafe { self.instance.get_physical_device_format_properties(self.pdev, format) };
        p.optimal_tiling_features.contains(need)
    }

    pub(crate) fn queue(&self) -> MutexGuard<'_, vk::Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits until every submitted frame finished and the queue is idle (presents included).
    fn wait_queue_idle(&self) {
        let q = self.queue();
        // SAFETY: the queue mutex is held (external sync).
        let _ = unsafe { self.raw.queue_wait_idle(*q) };
    }
}

pub(crate) fn range(aspect: vk::ImageAspectFlags, mip: u32, mips: u32, layer: u32, layers: u32) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange { aspect_mask: aspect, base_mip_level: mip, level_count: mips, base_array_layer: layer, layer_count: layers }
}

/// What a SPIR-V module says about one entry point.
struct SpirvEntry {
    local_size: Option<[u32; 3]>,
    /// Workgroup size comes from specialisation constants (`local_size_x_id` or the
    /// `WorkgroupSize` built-in), so the pipeline's `workgroup_size` is applied.
    specialised: bool,
}

/// Finds `entry` with the execution model of `stage` in a SPIR-V module.
fn spirv_entry(words: &[u32], entry: &str, stage: ShaderStage) -> std::result::Result<SpirvEntry, String> {
    if words.len() < 5 || words[0] != 0x0723_0203 {
        return Err("not a SPIR-V module (bad magic)".into());
    }
    let model = match stage {
        ShaderStage::Vertex => 0,
        ShaderStage::Fragment => 4,
        ShaderStage::Compute => 5,
    };
    let mut target = None;
    let mut models = Vec::new();
    let mut local_size = None;
    let mut local_size_id = false;
    let mut builtin = false;
    let mut modes = Vec::new();
    let mut i = 5;
    while i < words.len() {
        let wc = (words[i] >> 16) as usize;
        let op = words[i] & 0xffff;
        if wc == 0 || i + wc > words.len() {
            return Err("truncated SPIR-V module".into());
        }
        let ops = &words[i + 1..i + wc];
        match op {
            // OpEntryPoint model id name...
            15 if ops.len() >= 3 => {
                let bytes: Vec<u8> = ops[2..].iter().flat_map(|w| w.to_le_bytes()).collect();
                let name = bytes.split(|b| *b == 0).next().unwrap_or(&[]);
                if name == entry.as_bytes() {
                    models.push(ops[0]);
                    if ops[0] == model {
                        target = Some(ops[1]);
                    }
                }
            }
            // OpExecutionMode / OpExecutionModeId
            16 | 331 if ops.len() >= 2 => modes.push((op, ops[0], ops[1], ops[2..].to_vec())),
            // OpDecorate id BuiltIn WorkgroupSize
            71 if ops.len() >= 3 && ops[1] == 11 && ops[2] == 25 => builtin = true,
            _ => {}
        }
        i += wc;
    }
    let Some(id) = target else {
        return Err(if models.is_empty() { format!("no entry point '{entry}'") } else { format!("entry '{entry}' is not a {stage:?} shader (execution models {models:?})") });
    };
    for (op, eid, mode, lits) in modes {
        if eid != id {
            continue;
        }
        match (op, mode) {
            (16, 17) if lits.len() >= 3 => local_size = Some([lits[0], lits[1], lits[2]]),
            (331, 38) => local_size_id = true,
            _ => {}
        }
    }
    Ok(SpirvEntry { local_size, specialised: builtin || local_size_id })
}

#[cfg(target_os = "macos")]
fn metal_layer(window: &WindowHandle) -> Result<objc2::rc::Retained<objc2_quartz_core::CAMetalLayer>> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{ClassType, msg_send};
    use objc2_quartz_core::CAMetalLayer;
    let obj = std::ptr::NonNull::new(window.window as *mut AnyObject).ok_or_else(|| invalid("create_surface: null window"))?;
    // SAFETY: the caller passes a live NSView* or CAMetalLayer* (WindowHandle contract).
    let obj: &AnyObject = unsafe { obj.as_ref() };
    // SAFETY: isKindOfClass: is defined on every NSObject.
    let is_layer: bool = unsafe { msg_send![obj, isKindOfClass: CAMetalLayer::class()] };
    if is_layer {
        // SAFETY: just checked the class; retaining keeps it alive for the surface.
        unsafe { Retained::retain(obj as *const AnyObject as *mut CAMetalLayer) }.ok_or_else(|| invalid("create_surface: null layer"))
    } else {
        let layer = CAMetalLayer::new();
        // SAFETY: `obj` is an NSView (contract); main-thread NSView setters.
        unsafe {
            let _: () = msg_send![obj, setWantsLayer: true];
            let _: () = msg_send![obj, setLayer: &*layer];
        }
        Ok(layer)
    }
}

/// Logs a suboptimal swapchain once per swapchain; it keeps presenting.
fn note_suboptimal(s: &mut Surface, at: &str) {
    if !s.suboptimal_seen {
        s.suboptimal_seen = true;
        eprintln!("[ed_gpu vulkan] swapchain {}x{} is suboptimal ({at}); still presenting, reconfigure to match the window", s.config.width, s.config.height);
    }
}

impl VulkanDevice {
    /// Creates (or recreates, retiring `old`) the swapchain of a surface.
    fn build_swapchain(&self, s: &mut Surface, c: &SurfaceConfig) -> Result<()> {
        if c.width == 0 || c.height == 0 {
            return Err(invalid("surface size 0"));
        }
        if !matches!(c.format, Format::Bgra8Srgb | Format::Bgra8Unorm | Format::Rgb10A2Unorm | Format::Rgba16Float) {
            return Err(invalid(format!("surface format {:?} not presentable", c.format)));
        }
        if c.hdr && c.format != Format::Rgba16Float {
            return Err(invalid("HDR surfaces need Rgba16Float"));
        }
        #[cfg(target_os = "macos")]
        s._layer.setDrawableSize(objc2_core_foundation::CGSize { width: c.width as f64, height: c.height as f64 });
        let su = &self.exts.surface;
        // SAFETY: plain queries on a valid surface.
        let (caps, formats, modes) = unsafe {
            (
                su.get_physical_device_surface_capabilities(self.pdev, s.surface).map_err(vkr)?,
                su.get_physical_device_surface_formats(self.pdev, s.surface).map_err(vkr)?,
                su.get_physical_device_surface_present_modes(self.pdev, s.surface).map_err(vkr)?,
            )
        };
        let fmt = conv::format(c.format);
        let space = if c.hdr { vk::ColorSpaceKHR::EXTENDED_SRGB_LINEAR_EXT } else { vk::ColorSpaceKHR::SRGB_NONLINEAR };
        if c.hdr && !self.exts.colorspace {
            return Err(Error::Unsupported("HDR surfaces need VK_EXT_swapchain_colorspace".into()));
        }
        if !formats.iter().any(|f| f.format == fmt && f.color_space == space) {
            return Err(invalid(format!("surface format {:?} / {space:?} not offered by this surface", c.format)));
        }
        let mode = if c.vsync {
            vk::PresentModeKHR::FIFO
        } else {
            [vk::PresentModeKHR::IMMEDIATE, vk::PresentModeKHR::MAILBOX].into_iter().find(|m| modes.contains(m)).unwrap_or(vk::PresentModeKHR::FIFO)
        };
        let mut count = self.frames.max(caps.min_image_count);
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let mut usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
        for u in [vk::ImageUsageFlags::TRANSFER_SRC, vk::ImageUsageFlags::TRANSFER_DST, vk::ImageUsageFlags::SAMPLED] {
            if caps.supported_usage_flags.contains(u) {
                usage |= u;
            }
        }
        let alpha = [vk::CompositeAlphaFlagsKHR::OPAQUE, vk::CompositeAlphaFlagsKHR::INHERIT, vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED, vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED]
            .into_iter()
            .find(|a| caps.supported_composite_alpha.contains(*a))
            .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);
        let extent = vk::Extent2D {
            width: c.width.clamp(caps.min_image_extent.width, caps.max_image_extent.width.max(caps.min_image_extent.width)),
            height: c.height.clamp(caps.min_image_extent.height, caps.max_image_extent.height.max(caps.min_image_extent.height)),
        };
        if extent.width != c.width || extent.height != c.height {
            return Err(invalid(format!("surface size {}x{} outside {:?}..{:?}", c.width, c.height, caps.min_image_extent, caps.max_image_extent)));
        }
        let old = s.swapchain;
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(s.surface)
            .min_image_count(count)
            .image_format(fmt)
            .image_color_space(space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(alpha)
            .present_mode(mode)
            .clipped(true)
            .old_swapchain(old);
        // SAFETY: valid surface; the old swapchain (if any) has no acquired images (caller
        // waited for the queue to go idle).
        let sc = unsafe { self.exts.swapchain.create_swapchain(&info, None) }.map_err(vkr)?;
        self.drop_swapchain(s);
        s.swapchain = sc;
        // SAFETY: valid swapchain.
        s.images = unsafe { self.exts.swapchain.get_swapchain_images(sc) }.map_err(vkr)?;
        for (i, &img) in s.images.iter().enumerate() {
            s.views.push(self.make_view(img, vk::ImageViewType::TYPE_2D, fmt, vk::ImageAspectFlags::COLOR, 0, 1, 0, 1)?);
            // SAFETY: valid device.
            s.present_sems.push(unsafe { self.raw.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }.map_err(vkr)?);
            self.name(img, &format!("swapchain image {i}"));
        }
        s.config = c.clone();
        s.lost = false;
        s.suboptimal_seen = false;
        Ok(())
    }

    /// Destroys the swapchain, its views and present semaphores (queue must be idle).
    fn drop_swapchain(&self, s: &mut Surface) {
        // SAFETY: the caller made sure no submitted work uses them.
        unsafe {
            for v in s.views.drain(..) {
                self.raw.destroy_image_view(v, None);
            }
            for p in s.present_sems.drain(..) {
                self.raw.destroy_semaphore(p, None);
            }
            if s.swapchain != vk::SwapchainKHR::null() {
                self.exts.swapchain.destroy_swapchain(s.swapchain, None);
            }
        }
        s.swapchain = vk::SwapchainKHR::null();
        s.images.clear();
    }

    fn destroy_surface_now(&self, mut s: Surface) {
        self.drop_swapchain(&mut s);
        // SAFETY: queue idle; nothing uses these.
        unsafe {
            for a in s.acquire_sems.drain(..) {
                self.raw.destroy_semaphore(a, None);
            }
            self.exts.surface.destroy_surface(s.surface, None);
        }
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        self.wait_idle();
        self.wait_queue_idle();
        let st = self.state.0.get_mut().unwrap_or_else(|e| e.into_inner());
        let mut st = std::mem::replace(
            st,
            State {
                buffers: Pool::default(),
                textures: Pool::default(),
                samplers: Pool::default(),
                shaders: Pool::default(),
                rpipes: Pool::default(),
                cpipes: Pool::default(),
                surfaces: Pool::default(),
                tables: Tables {
                    tex_slots: Slots::new(0),
                    tex_owner: Vec::new(),
                    tex_storage: Vec::new(),
                    buf_slots: Slots::new(0),
                    buf_owner: Vec::new(),
                    smp_slots: Slots::new(0),
                    smp_owner: Vec::new(),
                },
                frames_begun: 0,
                submitted: 0,
                completed: 0,
                frame_open: false,
                in_flight: VecDeque::new(),
                deferred: Vec::new(),
                uploads: Vec::new(),
                acquired: Vec::new(),
                slots: Vec::new(),
                timings: Vec::new(),
            },
        );
        for (_, d) in st.deferred.drain(..) {
            self.free_deferred(d);
        }
        for b in st.buffers.drain() {
            self.free_deferred(Deferred::Buffer(b));
        }
        for t in st.textures.drain() {
            self.free_deferred(Deferred::Texture(t));
        }
        for s in st.samplers.drain() {
            self.free_deferred(Deferred::Sampler(s));
        }
        for p in st.rpipes.drain() {
            self.free_deferred(Deferred::Pipeline(p.raw));
        }
        for p in st.cpipes.drain() {
            self.free_deferred(Deferred::Pipeline(p.raw));
        }
        for s in st.surfaces.drain() {
            self.destroy_surface_now(s);
        }
        // SAFETY: the device is idle; every object below was created by it and is unused.
        unsafe {
            for s in st.shaders.drain() {
                self.raw.destroy_shader_module(s.module, None);
            }
            for s in st.slots.drain(..) {
                self.raw.destroy_command_pool(s.pool, None);
                self.raw.destroy_query_pool(s.queries, None);
                if let Some(b) = s.staging {
                    self.free_buffer(b);
                }
                self.free_buffer(s.addrs);
            }
            let (img, mem, view) = self.dummy_image;
            if view != vk::ImageView::null() {
                self.raw.destroy_image_view(view, None);
            }
            if img != vk::Image::null() {
                self.raw.destroy_image(img, None);
            }
            self.free_mem(mem, 0);
            let db = std::mem::replace(&mut self.dummy_buffer, RawBuffer { raw: vk::Buffer::null(), mem: vk::DeviceMemory::null(), ptr: std::ptr::null_mut(), bytes: 0 });
            self.free_buffer(db);
            if self.dummy_sampler != vk::Sampler::null() {
                self.raw.destroy_sampler(self.dummy_sampler, None);
            }
            self.raw.destroy_descriptor_pool(self.desc_pool, None);
            self.raw.destroy_pipeline_layout(self.layout, None);
            self.raw.destroy_descriptor_set_layout(self.set_layout, None);
            self.raw.destroy_semaphore(self.timeline, None);
            self.raw.destroy_device(None);
            if let Some((di, m)) = self.debug_instance.take() {
                di.destroy_debug_utils_messenger(m, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

impl Device for VulkanDevice {
    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn create_buffer(&self, desc: &BufferDesc<'_>) -> Result<BufferId> {
        if desc.size == 0 {
            return Err(invalid(format!("buffer '{}': size 0", desc.label)));
        }
        let u = desc.usage;
        if u.cpu_write && u.cpu_read {
            return Err(invalid(format!("buffer '{}': cpu_write and cpu_read together are not supported", desc.label)));
        }
        let mut flags = vk::BufferUsageFlags::empty();
        if u.shader_read || u.shader_write || u.cpu_write {
            flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
        }
        if u.shader_read {
            flags |= vk::BufferUsageFlags::INDEX_BUFFER;
        }
        if u.indirect_args {
            flags |= vk::BufferUsageFlags::INDIRECT_BUFFER;
        }
        if u.copy_src {
            flags |= vk::BufferUsageFlags::TRANSFER_SRC;
        }
        if u.copy_dst {
            flags |= vk::BufferUsageFlags::TRANSFER_DST;
        }
        if flags.is_empty() {
            flags = vk::BufferUsageFlags::TRANSFER_DST;
        }
        let kind = if u.cpu_write {
            MemKind::Upload
        } else if u.cpu_read {
            MemKind::Readback
        } else {
            MemKind::Gpu
        };
        let n = if u.cpu_write { self.frames } else { 1 };
        let mut copies = Vec::new();
        for i in 0..n {
            let label = if n > 1 { format!("{} [{i}]", desc.label) } else { desc.label.to_string() };
            match self.make_buffer(desc.size, flags, kind, &label) {
                Ok(b) => copies.push(b),
                Err(e) => {
                    copies.into_iter().for_each(|c| self.free_buffer(c));
                    return Err(e);
                }
            }
        }
        let (i, g) = self.lock().buffers.insert(Buffer { copies, size: desc.size, usage: u, states: States::new(1, 1), bound: None });
        Ok(BufferId::from_parts(i, g))
    }

    fn create_texture(&self, desc: &TextureDesc<'_>) -> Result<TextureId> {
        let layers = match desc.kind {
            TextureKind::Cube if desc.depth_or_layers != 6 => return Err(invalid(format!("texture '{}': cube needs depth_or_layers 6", desc.label))),
            TextureKind::D2 if desc.depth_or_layers != 1 => return Err(invalid(format!("texture '{}': D2 needs depth_or_layers 1", desc.label))),
            TextureKind::D2Array | TextureKind::Cube => desc.depth_or_layers,
            _ => 1,
        };
        let max_mips = 32 - desc.width.max(desc.height).max(1).leading_zeros();
        if desc.width == 0 || desc.height == 0 || desc.depth_or_layers == 0 || desc.mips == 0 || desc.mips > max_mips {
            return Err(invalid(format!("texture '{}': bad size or mip count", desc.label)));
        }
        let u = desc.usage;
        if u.cpu_read || u.cpu_write {
            return Err(invalid(format!("texture '{}': CPU access goes through buffers", desc.label)));
        }
        if u.depth != desc.format.is_depth() {
            return Err(invalid(format!("texture '{}': usage.depth must match a depth format", desc.label)));
        }
        let fmt = conv::format(desc.format);
        let mut flags = vk::ImageUsageFlags::empty();
        let mut need = vk::FormatFeatureFlags::empty();
        if u.shader_read || u.depth {
            flags |= vk::ImageUsageFlags::SAMPLED;
            need |= vk::FormatFeatureFlags::SAMPLED_IMAGE;
        }
        if u.shader_write {
            flags |= vk::ImageUsageFlags::STORAGE;
            need |= vk::FormatFeatureFlags::STORAGE_IMAGE;
        }
        if u.render_target {
            flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
            need |= vk::FormatFeatureFlags::COLOR_ATTACHMENT;
        }
        if u.depth {
            flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
            need |= vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT;
        }
        if u.copy_src {
            flags |= vk::ImageUsageFlags::TRANSFER_SRC;
        }
        if u.copy_dst {
            flags |= vk::ImageUsageFlags::TRANSFER_DST;
            // generate_mips blits from one mip to the next.
            if desc.mips > 1 {
                flags |= vk::ImageUsageFlags::TRANSFER_SRC;
            }
        }
        if !self.format_supports(fmt, need) {
            return Err(Error::Unsupported(format!("texture '{}': format {:?} does not support {need:?} on this device", desc.label, desc.format)));
        }
        let samples = vk::SampleCountFlags::from_raw(desc.samples.max(1));
        let ici = vk::ImageCreateInfo::default()
            .image_type(if desc.kind == TextureKind::D3 { vk::ImageType::TYPE_3D } else { vk::ImageType::TYPE_2D })
            .format(fmt)
            .extent(vk::Extent3D { width: desc.width, height: desc.height, depth: if desc.kind == TextureKind::D3 { desc.depth_or_layers } else { 1 } })
            .mip_levels(desc.mips)
            .array_layers(layers)
            .samples(samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(flags)
            .flags(if desc.kind == TextureKind::Cube { vk::ImageCreateFlags::CUBE_COMPATIBLE } else { vk::ImageCreateFlags::empty() });
        // SAFETY: valid device and create info (sizes validated above; Vulkan checks limits).
        let image = unsafe { self.raw.create_image(&ici, None) }.map_err(vkr)?;
        // SAFETY: valid image.
        let req = unsafe { self.raw.get_image_memory_requirements(image) };
        let mut tex = Texture {
            image,
            mem: vk::DeviceMemory::null(),
            bytes: req.size,
            view: vk::ImageView::null(),
            storage_view: vk::ImageView::null(),
            att_views: Vec::new(),
            kind: desc.kind,
            format: desc.format,
            width: desc.width,
            height: desc.height,
            depth_or_layers: desc.depth_or_layers,
            mips: desc.mips,
            usage: u,
            states: States::new(desc.mips, layers),
            bound: None,
            surface: false,
            sampled_layout: conv::sampled_layout(&u, desc.format),
        };
        let built = (|| -> Result<()> {
            let (mem, _) = self.alloc(req, MemKind::Gpu, Some(desc.label))?;
            tex.mem = mem;
            // SAFETY: fresh image and memory of the required size.
            unsafe { self.raw.bind_image_memory(image, mem, 0) }.map_err(vkr)?;
            let aspect = conv::aspect(desc.format);
            tex.view = self.make_view(image, conv::view_type(desc.kind), fmt, aspect, 0, desc.mips, 0, layers)?;
            if u.shader_write && desc.kind == TextureKind::D2 {
                tex.storage_view = self.make_view(image, vk::ImageViewType::TYPE_2D, fmt, aspect, 0, 1, 0, 1)?;
            }
            if (u.render_target || u.depth) && desc.kind != TextureKind::D3 {
                for layer in 0..layers {
                    for mip in 0..desc.mips {
                        tex.att_views.push(self.make_view(image, vk::ImageViewType::TYPE_2D, fmt, aspect, mip, 1, layer, 1)?);
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = built {
            self.free_texture(tex);
            return Err(e);
        }
        self.name(image, desc.label);
        if let Err(e) = self.check_validation(&format!("texture '{}'", desc.label)) {
            self.free_texture(tex);
            return Err(e);
        }
        let (i, g) = self.lock().textures.insert(tex);
        Ok(TextureId::from_parts(i, g))
    }

    fn create_sampler(&self, desc: &SamplerDesc) -> Result<SamplerId> {
        let a = conv::address(desc.address);
        let aniso = desc.anisotropy.clamp(1, 16) as f32;
        let mut info = vk::SamplerCreateInfo::default()
            .min_filter(conv::filter(desc.min_mag))
            .mag_filter(conv::filter(desc.min_mag))
            .mipmap_mode(conv::mip(desc.mip))
            .address_mode_u(a)
            .address_mode_v(a)
            .address_mode_w(a)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE);
        if aniso > 1.0 {
            info = info.anisotropy_enable(true).max_anisotropy(aniso);
        }
        if let Some(c) = desc.compare {
            info = info.compare_enable(true).compare_op(conv::compare(c));
        }
        // SAFETY: valid device and create info.
        let raw = unsafe { self.raw.create_sampler(&info, None) }.map_err(vkr)?;
        let (i, g) = self.lock().samplers.insert(Sampler { raw, bound: None });
        Ok(SamplerId::from_parts(i, g))
    }

    fn create_shader(&self, desc: &ShaderDesc<'_>) -> Result<ShaderId> {
        let ShaderCode::SpirV(words) = desc.code else {
            return Err(invalid(format!("shader '{}': the Vulkan backend takes SpirV, not MSL, metallib or DXIL", desc.label)));
        };
        let e = spirv_entry(words, desc.entry, desc.stage).map_err(|m| invalid(format!("shader '{}': {m}", desc.label)))?;
        let entry = CString::new(desc.entry).map_err(|_| invalid(format!("shader '{}': entry name has a NUL", desc.label)))?;
        // SAFETY: valid device; the code is a checked SPIR-V header with whole instructions.
        let module = unsafe { self.raw.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(words), None) }.map_err(vkr)?;
        if let Err(err) = self.check_validation(&format!("shader '{}'", desc.label)) {
            // SAFETY: unused module.
            unsafe { self.raw.destroy_shader_module(module, None) };
            return Err(err);
        }
        self.name(module, desc.label);
        let local_size = if e.specialised { None } else { e.local_size };
        let (i, g) = self.lock().shaders.insert(Shader { module, stage: desc.stage, entry, local_size });
        Ok(ShaderId::from_parts(i, g))
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDesc<'_>) -> Result<RenderPipelineId> {
        desc.validate()?;
        let (vs, fs) = {
            let st = self.lock();
            let vs = st.shaders.get(desc.vertex.index(), desc.vertex.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale vertex shader", desc.label)))?;
            if vs.stage != ShaderStage::Vertex {
                return Err(invalid(format!("pipeline '{}': vertex shader has stage {:?}", desc.label, vs.stage)));
            }
            let fs = match desc.fragment {
                Some(f) => {
                    let s = st.shaders.get(f.index(), f.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale fragment shader", desc.label)))?;
                    if s.stage != ShaderStage::Fragment {
                        return Err(invalid(format!("pipeline '{}': fragment shader has stage {:?}", desc.label, s.stage)));
                    }
                    Some((s.module, s.entry.clone()))
                }
                None => None,
            };
            ((vs.module, vs.entry.clone()), fs)
        };
        if desc.color_formats.len() > 8 {
            return Err(invalid(format!("pipeline '{}': more than 8 colour targets", desc.label)));
        }
        if let Some(d) = &desc.depth
            && !d.format.is_depth()
        {
            return Err(invalid(format!("pipeline '{}': depth format {:?} is not a depth format", desc.label, d.format)));
        }
        let mut stages = vec![vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs.0).name(&vs.1)];
        if let Some((m, e)) = &fs {
            stages.push(vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(*m).name(e));
        }
        let vin = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(conv::topology(desc.topology));
        let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let (bias, slope) = desc.depth.as_ref().map(|d| (d.bias, d.slope_bias)).unwrap_or((0.0, 0.0));
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(conv::cull(desc.cull))
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0)
            .depth_clamp_enable(self.depth_clamp && desc.depth.as_ref().is_some_and(|d| d.clamp))
            .depth_bias_enable(bias != 0.0 || slope != 0.0)
            .depth_bias_constant_factor(bias)
            .depth_bias_slope_factor(slope);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::from_raw(desc.samples.max(1)));
        let ds = match &desc.depth {
            Some(d) => vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(d.write).depth_compare_op(conv::compare(d.compare)),
            None => vk::PipelineDepthStencilStateCreateInfo::default(),
        };
        let blends: Vec<_> = desc.color_formats.iter().map(|_| conv::blend(desc.blend)).collect();
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blends);
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let color_fmts: Vec<_> = desc.color_formats.iter().map(|f| conv::format(*f)).collect();
        let mut rendering = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_fmts)
            .depth_attachment_format(desc.depth.as_ref().map(|d| conv::format(d.format)).unwrap_or(vk::Format::UNDEFINED));
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vin)
            .input_assembly_state(&ia)
            .viewport_state(&vp)
            .rasterization_state(&rs)
            .multisample_state(&ms)
            .depth_stencil_state(&ds)
            .color_blend_state(&cb)
            .dynamic_state(&dynamic)
            .layout(self.layout)
            .push_next(&mut rendering);
        // Blending is off on every unblendable target here (`validate` above), so MoltenVK's
        // blend warning during this call is the known false positive.
        let unblendable = desc.color_formats.iter().any(|f| !f.is_blendable());
        let flag = &self.messages.unblendable_opaque;
        if unblendable {
            flag.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        // SAFETY: every pointer in the create info is alive for the call; modules are valid.
        let created = unsafe { self.raw.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) };
        if unblendable {
            flag.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
        let raw = created.map_err(|(_, e)| invalid(format!("pipeline '{}': {e:?}", desc.label)))?[0];
        if let Err(e) = self.check_validation(&format!("pipeline '{}'", desc.label)) {
            // SAFETY: unused pipeline.
            unsafe { self.raw.destroy_pipeline(raw, None) };
            return Err(e);
        }
        self.name(raw, desc.label);
        let pipe = RenderPipe { raw, colors: desc.color_formats.to_vec(), depth: desc.depth.as_ref().map(|d| (d.format, d.write)) };
        let (i, g) = self.lock().rpipes.insert(pipe);
        Ok(RenderPipelineId::from_parts(i, g))
    }

    fn create_compute_pipeline(&self, desc: &ComputePipelineDesc<'_>) -> Result<ComputePipelineId> {
        let (module, entry, local) = {
            let st = self.lock();
            let s = st.shaders.get(desc.shader.index(), desc.shader.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale shader", desc.label)))?;
            if s.stage != ShaderStage::Compute {
                return Err(invalid(format!("pipeline '{}': shader has stage {:?}", desc.label, s.stage)));
            }
            (s.module, s.entry.clone(), s.local_size)
        };
        let [x, y, z] = desc.workgroup_size;
        let threads = x as u64 * y as u64 * z as u64;
        if threads == 0 || threads > self.caps.max_workgroup_threads as u64 {
            return Err(invalid(format!("pipeline '{}': workgroup size {:?}", desc.label, desc.workgroup_size)));
        }
        if let Some(l) = local
            && l != desc.workgroup_size
        {
            return Err(invalid(format!("pipeline '{}': workgroup_size {:?} does not match the shader's local_size {l:?}", desc.label, desc.workgroup_size)));
        }
        // Specialisation constants 0, 1, 2 carry the workgroup size (`local_size_x_id = 0`...).
        let data: Vec<u8> = desc.workgroup_size.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let map = [0u32, 1, 2].map(|i| vk::SpecializationMapEntry { constant_id: i, offset: i * 4, size: 4 });
        let spec = vk::SpecializationInfo::default().map_entries(&map).data(&data);
        let stage = vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::COMPUTE).module(module).name(&entry).specialization_info(&spec);
        let info = vk::ComputePipelineCreateInfo::default().stage(stage).layout(self.layout);
        // SAFETY: every pointer in the create info is alive for the call.
        let raw = unsafe { self.raw.create_compute_pipelines(vk::PipelineCache::null(), &[info], None) }
            .map_err(|(_, e)| invalid(format!("pipeline '{}': {e:?}", desc.label)))?[0];
        if let Err(e) = self.check_validation(&format!("pipeline '{}'", desc.label)) {
            // SAFETY: unused pipeline.
            unsafe { self.raw.destroy_pipeline(raw, None) };
            return Err(e);
        }
        self.name(raw, desc.label);
        let (i, g) = self.lock().cpipes.insert(ComputePipe { raw });
        Ok(ComputePipelineId::from_parts(i, g))
    }

    fn destroy_buffer(&self, id: BufferId) {
        let mut st = self.lock();
        if let Some(b) = st.buffers.remove(id.index(), id.generation()) {
            if let Some(slot) = b.bound {
                st.tables.buf_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.buf_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Buffer(b));
            self.retire(&mut st);
        }
    }

    fn destroy_texture(&self, id: TextureId) {
        let mut st = self.lock();
        if let Some(t) = st.textures.get(id.index(), id.generation())
            && t.surface
        {
            return; // Surface textures belong to the frame.
        }
        if let Some(t) = st.textures.remove(id.index(), id.generation()) {
            if let Some(slot) = t.bound {
                st.tables.tex_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.tex_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Texture(t));
            self.retire(&mut st);
        }
    }

    fn destroy_sampler(&self, id: SamplerId) {
        let mut st = self.lock();
        if let Some(s) = st.samplers.remove(id.index(), id.generation()) {
            if let Some(slot) = s.bound {
                st.tables.smp_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.smp_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Sampler(s));
            self.retire(&mut st);
        }
    }

    fn destroy_shader(&self, id: ShaderId) {
        if let Some(s) = self.lock().shaders.remove(id.index(), id.generation()) {
            // SAFETY: pipelines do not need their modules after creation.
            unsafe { self.raw.destroy_shader_module(s.module, None) };
        }
    }

    fn destroy_render_pipeline(&self, id: RenderPipelineId) {
        let mut st = self.lock();
        if let Some(p) = st.rpipes.remove(id.index(), id.generation()) {
            self.defer(&mut st, Deferred::Pipeline(p.raw));
        }
    }

    fn destroy_compute_pipeline(&self, id: ComputePipelineId) {
        let mut st = self.lock();
        if let Some(p) = st.cpipes.remove(id.index(), id.generation()) {
            self.defer(&mut st, Deferred::Pipeline(p.raw));
        }
    }

    fn bind_texture(&self, id: TextureId) -> Result<BindlessTexture> {
        let mut st = self.lock();
        let st = &mut *st;
        let t = st.textures.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_texture: stale texture"))?;
        if let Some(s) = t.bound {
            return Ok(BindlessTexture(s));
        }
        if t.surface {
            return Err(invalid("bind_texture: surface textures cannot be bound"));
        }
        if !(t.usage.shader_read || t.usage.shader_write || t.usage.depth) {
            return Err(invalid("bind_texture: texture has neither shader_read nor shader_write usage"));
        }
        let slot = st.tables.tex_slots.alloc().ok_or_else(|| invalid("bind_texture: texture table full"))?;
        t.bound = Some(slot);
        let sampled = (t.usage.shader_read || t.usage.depth || t.usage.shader_write).then(|| vk::DescriptorImageInfo::default().image_view(t.view).image_layout(t.sampled_layout));
        let storage = (t.storage_view != vk::ImageView::null()).then(|| vk::DescriptorImageInfo::default().image_view(t.storage_view).image_layout(vk::ImageLayout::GENERAL));
        if st.tables.tex_owner.len() <= slot as usize {
            st.tables.tex_owner.resize(slot as usize + 1, None);
            st.tables.tex_storage.resize(slot as usize + 1, false);
        }
        st.tables.tex_owner[slot as usize] = Some(id);
        st.tables.tex_storage[slot as usize] = storage.is_some();
        for f in 0..st.slots.len() {
            let set = st.slots[f].set;
            if let Some(s) = sampled {
                self.write_descriptor(set, B_TEXTURES, slot, Some(s), None);
            }
            if let Some(s) = storage {
                self.write_descriptor(set, B_IMAGES, slot, Some(s), None);
            }
        }
        Ok(BindlessTexture(slot))
    }

    fn bind_buffer(&self, id: BufferId) -> Result<BindlessBuffer> {
        let mut st = self.lock();
        let st = &mut *st;
        let b = st.buffers.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_buffer: stale buffer"))?;
        if let Some(s) = b.bound {
            return Ok(BindlessBuffer(s));
        }
        if !(b.usage.shader_read || b.usage.shader_write || b.usage.cpu_write) {
            return Err(invalid("bind_buffer: buffer has no shader usage"));
        }
        let slot = st.tables.buf_slots.alloc().ok_or_else(|| invalid("bind_buffer: buffer table full"))?;
        b.bound = Some(slot);
        let raws: Vec<_> = b.copies.iter().map(|c| c.raw).collect();
        if st.tables.buf_owner.len() <= slot as usize {
            st.tables.buf_owner.resize(slot as usize + 1, None);
        }
        st.tables.buf_owner[slot as usize] = Some(id);
        for f in 0..st.slots.len() {
            let raw = raws[f % raws.len()];
            let info = vk::DescriptorBufferInfo::default().buffer(raw).range(vk::WHOLE_SIZE);
            self.write_descriptor(st.slots[f].set, B_BUFFERS, slot, None, Some(info));
            // SAFETY: valid buffer created with SHADER_DEVICE_ADDRESS usage (it has STORAGE_BUFFER).
            let addr = unsafe { self.raw.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(raw)) };
            st.slots[f].set_addr(slot, addr);
        }
        Ok(BindlessBuffer(slot))
    }

    fn bind_sampler(&self, id: SamplerId) -> Result<BindlessSampler> {
        let mut st = self.lock();
        let st = &mut *st;
        let s = st.samplers.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_sampler: stale sampler"))?;
        if let Some(i) = s.bound {
            return Ok(BindlessSampler(i));
        }
        let slot = st.tables.smp_slots.alloc().ok_or_else(|| invalid("bind_sampler: sampler table full"))?;
        s.bound = Some(slot);
        let raw = s.raw;
        if st.tables.smp_owner.len() <= slot as usize {
            st.tables.smp_owner.resize(slot as usize + 1, None);
        }
        st.tables.smp_owner[slot as usize] = Some(id);
        for f in 0..st.slots.len() {
            self.write_descriptor(st.slots[f].set, B_SAMPLERS, slot, Some(vk::DescriptorImageInfo::default().sampler(raw)), None);
        }
        Ok(BindlessSampler(slot))
    }

    fn unbind_texture(&self, index: BindlessTexture) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.tex_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(t) = st.textures.get_mut(id.index(), id.generation()) {
                t.bound = None;
            }
            st.tables.tex_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn unbind_buffer(&self, index: BindlessBuffer) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.buf_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(b) = st.buffers.get_mut(id.index(), id.generation()) {
                b.bound = None;
            }
            st.tables.buf_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn unbind_sampler(&self, index: BindlessSampler) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.smp_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(s) = st.samplers.get_mut(id.index(), id.generation()) {
                s.bound = None;
            }
            st.tables.smp_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn write_buffer(&self, id: BufferId, offset: u64, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut st = self.lock();
        let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("write_buffer: stale buffer"))?;
        if !b.usage.copy_dst {
            return Err(invalid("write_buffer: buffer lacks copy_dst usage"));
        }
        if b.usage.cpu_write {
            return Err(invalid("write_buffer: use fill_frame_buffer for cpu_write buffers"));
        }
        if offset + data.len() as u64 > b.size || !offset.is_multiple_of(4) {
            return Err(invalid(format!("write_buffer: range {offset}+{} outside size {} or unaligned", data.len(), b.size)));
        }
        st.uploads.push(Upload::Buffer { data: data.to_vec(), dst: id, offset });
        Ok(())
    }

    fn write_texture(&self, id: TextureId, sub: Subresource, data: &[u8]) -> Result<()> {
        let mut st = self.lock();
        let t = st.textures.get(id.index(), id.generation()).ok_or_else(|| invalid("write_texture: stale texture"))?;
        if !t.usage.copy_dst {
            return Err(invalid("write_texture: texture lacks copy_dst usage"));
        }
        if sub.mip >= t.mips || sub.layer >= t.layers() {
            return Err(invalid("write_texture: subresource out of range"));
        }
        let (w, h) = (mip_size(t.width, sub.mip), mip_size(t.height, sub.mip));
        let d = if t.kind == TextureKind::D3 { mip_size(t.depth(), sub.mip) } else { 1 };
        let need = t.format.packed_row_bytes(w) as usize * t.format.block_rows(h) as usize * d as usize;
        if data.len() != need {
            return Err(invalid(format!("write_texture: {} bytes given, subresource needs {need}", data.len())));
        }
        st.uploads.push(Upload::Texture { data: data.to_vec(), dst: id, sub });
        Ok(())
    }

    fn fill_frame_buffer(&self, id: BufferId, fill: &mut dyn FnMut(&mut [u8])) -> Result<()> {
        let (ptr, size) = {
            let st = self.lock();
            if !st.frame_open {
                return Err(invalid("fill_frame_buffer: no frame is open"));
            }
            let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("fill_frame_buffer: stale buffer"))?;
            if !b.usage.cpu_write {
                return Err(invalid("fill_frame_buffer: buffer lacks cpu_write usage"));
            }
            let slot = ((st.frames_begun - 1) % self.frames as u64) as usize;
            (b.copies[slot].ptr, b.size as usize)
        };
        // SAFETY: persistently mapped coherent memory of at least `size` bytes; this slot's
        // previous frame has finished (begin_frame waited), so the GPU is not reading it; the
        // render thread is the only writer, and the slice lives only for the call.
        let bytes = unsafe { std::slice::from_raw_parts_mut(ptr, size) };
        fill(bytes);
        Ok(())
    }

    fn read_buffer(&self, id: BufferId, out: &mut Vec<u8>) -> Result<()> {
        let st = self.lock();
        let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("read_buffer: stale buffer"))?;
        if !b.usage.cpu_read {
            return Err(invalid("read_buffer: buffer lacks cpu_read usage"));
        }
        out.clear();
        // SAFETY: persistently mapped memory of `size` bytes, alive while the lock holds the buffer.
        let bytes = unsafe { std::slice::from_raw_parts(b.copies[0].ptr as *const u8, b.size as usize) };
        out.extend_from_slice(bytes);
        Ok(())
    }

    fn create_surface(&self, window: WindowHandle, config: &SurfaceConfig) -> Result<SurfaceId> {
        #[cfg(target_os = "macos")]
        {
            let ms = self.exts.metal_surface.as_ref().ok_or_else(|| Error::Unsupported("VK_EXT_metal_surface not available".into()))?;
            let layer = metal_layer(&window)?;
            let info = vk::MetalSurfaceCreateInfoEXT::default().layer(objc2::rc::Retained::as_ptr(&layer) as *const vk::CAMetalLayer);
            // SAFETY: the layer is alive (retained by the surface record).
            let surface = unsafe { ms.create_metal_surface(&info, None) }.map_err(vkr)?;
            // SAFETY: plain query.
            let ok = unsafe { self.exts.surface.get_physical_device_surface_support(self.pdev, self.queue_family, surface) }.unwrap_or(false);
            let mut s = Surface {
                surface,
                swapchain: vk::SwapchainKHR::null(),
                images: Vec::new(),
                views: Vec::new(),
                present_sems: Vec::new(),
                acquire_sems: Vec::new(),
                config: config.clone(),
                lost: false,
                suboptimal_seen: false,
                _layer: layer,
            };
            if !ok {
                self.destroy_surface_now(s);
                return Err(Error::Unsupported("the queue cannot present to this surface".into()));
            }
            for _ in 0..self.frames {
                // SAFETY: valid device.
                match unsafe { self.raw.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) } {
                    Ok(a) => s.acquire_sems.push(a),
                    Err(e) => {
                        self.destroy_surface_now(s);
                        return Err(vkr(e));
                    }
                }
            }
            if let Err(e) = self.build_swapchain(&mut s, config) {
                self.destroy_surface_now(s);
                return Err(e);
            }
            let (i, g) = self.lock().surfaces.insert(s);
            Ok(SurfaceId::from_parts(i, g))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (window, config);
            Err(Error::Unsupported("Vulkan surfaces are only wired for CAMetalLayer (macOS) so far".into()))
        }
    }

    fn configure_surface(&self, surface: SurfaceId, config: &SurfaceConfig) -> Result<()> {
        {
            let st = self.lock();
            st.surfaces.get(surface.index(), surface.generation()).ok_or_else(|| invalid("configure_surface: stale surface"))?;
            if st.frame_open && !st.acquired.is_empty() {
                return Err(invalid("configure_surface: reconfigure between frames"));
            }
        }
        self.wait_idle();
        self.wait_queue_idle();
        let mut st = self.lock();
        let s = st.surfaces.get_mut(surface.index(), surface.generation()).ok_or_else(|| invalid("configure_surface: stale surface"))?;
        self.build_swapchain(s, config)
    }

    fn destroy_surface(&self, surface: SurfaceId) {
        self.wait_idle();
        self.wait_queue_idle();
        let s = self.lock().surfaces.remove(surface.index(), surface.generation());
        if let Some(s) = s {
            self.destroy_surface_now(s);
        }
    }

    fn begin_frame(&self) -> Result<Box<dyn CommandEncoder + '_>> {
        loop {
            let wait_on = {
                let mut st = self.lock();
                if st.frame_open {
                    return Err(invalid("begin_frame: the previous frame was not ended"));
                }
                self.retire(&mut st);
                if st.in_flight.len() < self.frames as usize {
                    break;
                }
                st.in_flight.front().map(|f| f.index + 1)
            };
            if let Some(v) = wait_on {
                self.wait_value(v);
            }
        }
        encoder::VulkanEncoder::begin(self).map(|e| Box::new(e) as Box<dyn CommandEncoder + '_>)
    }

    fn acquire(&self, surface: SurfaceId) -> Result<TextureId> {
        let (swapchain, sem, config, images, views) = {
            let st = self.lock();
            if !st.frame_open {
                return Err(invalid("acquire: no frame is open"));
            }
            if st.acquired.iter().any(|a| a.0 == surface) {
                return Err(invalid("acquire: surface already acquired this frame"));
            }
            let s = st.surfaces.get(surface.index(), surface.generation()).ok_or_else(|| invalid("acquire: stale surface"))?;
            if s.lost {
                return Err(Error::SurfaceLost);
            }
            let slot = ((st.frames_begun - 1) % self.frames as u64) as usize;
            (s.swapchain, s.acquire_sems[slot], s.config.clone(), s.images.clone(), s.views.clone())
        };
        // SAFETY: valid swapchain and semaphore; the semaphore's previous wait (this slot's
        // previous frame) has completed.
        let r = unsafe { self.exts.swapchain.acquire_next_image(swapchain, u64::MAX, sem, vk::Fence::null()) };
        let (index, suboptimal) = match r {
            Ok(v) => v,
            Err(e) => {
                if let Some(s) = self.lock().surfaces.get_mut(surface.index(), surface.generation()) {
                    s.lost = true;
                }
                return Err(match e {
                    vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::ERROR_SURFACE_LOST_KHR => Error::SurfaceLost,
                    e => vkr(e),
                });
            }
        };
        let tex = Texture {
            image: images[index as usize],
            mem: vk::DeviceMemory::null(),
            bytes: 0,
            view: views[index as usize],
            storage_view: vk::ImageView::null(),
            att_views: vec![views[index as usize]],
            kind: TextureKind::D2,
            format: config.format,
            width: config.width,
            height: config.height,
            depth_or_layers: 1,
            mips: 1,
            usage: Usage { render_target: true, copy_src: true, copy_dst: true, shader_read: true, ..Default::default() },
            states: States::new(1, 1),
            bound: None,
            surface: true,
            sampled_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        };
        let mut st = self.lock();
        if suboptimal && let Some(s) = st.surfaces.get_mut(surface.index(), surface.generation()) {
            note_suboptimal(s, "acquire");
        }
        let (i, g) = st.textures.insert(tex);
        let id = TextureId::from_parts(i, g);
        st.acquired.push((surface, index, id));
        Ok(id)
    }

    fn end_frame(&self, encoder: Box<dyn CommandEncoder + '_>) -> Result<()> {
        // `finish` turns validation errors raised while the frame was recorded or submitted
        // into `Err` ("Vulkan validation: ..."); under ED_GPU_VK_VALIDATION=require they panic
        // here, so no caller can drop them.
        let r = encoder.finish();
        if let Err(e) = &r
            && self.require_validation
            && e.to_string().contains("Vulkan validation:")
        {
            panic!("{e} (ED_GPU_VK_VALIDATION=require)");
        }
        r
    }

    fn wait_idle(&self) {
        let submitted = self.lock().submitted;
        if submitted > 0 {
            self.wait_value(submitted);
        }
        let mut st = self.lock();
        self.retire(&mut st);
    }

    fn completed_frames(&self) -> u64 {
        let mut st = self.lock();
        self.retire(&mut st);
        st.completed
    }

    fn timings(&self, out: &mut Vec<Timing>) {
        let mut st = self.lock();
        self.retire(&mut st);
        out.clear();
        out.extend_from_slice(&st.timings);
    }

    fn allocated_bytes(&self) -> u64 {
        self.allocated.load(Ordering::Relaxed)
    }
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn f<T: Send + Sync>() {}
    f::<VulkanDevice>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(entries: &[(u32, &str)], modes: &[(u32, u32, &[u32])]) -> Vec<u32> {
        let mut w = vec![0x0723_0203, 0x0001_0600, 0, 100, 0];
        for (i, (model, name)) in entries.iter().enumerate() {
            let mut bytes = name.as_bytes().to_vec();
            bytes.push(0);
            while bytes.len() % 4 != 0 {
                bytes.push(0);
            }
            let words: Vec<u32> = bytes.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            w.push(((3 + words.len() as u32) << 16) | 15);
            w.push(*model);
            w.push(10 + i as u32);
            w.extend(words);
        }
        for (id, mode, lits) in modes {
            w.push(((3 + lits.len() as u32) << 16) | 16);
            w.push(*id);
            w.push(*mode);
            w.extend_from_slice(lits);
        }
        w
    }

    #[test]
    fn spirv_entry_points_and_local_size() {
        let m = module(&[(5, "main"), (0, "vs")], &[(10, 17, &[64, 1, 1])]);
        let e = spirv_entry(&m, "main", ShaderStage::Compute).unwrap();
        assert_eq!(e.local_size, Some([64, 1, 1]));
        assert!(!e.specialised);
        assert!(spirv_entry(&m, "vs", ShaderStage::Vertex).is_ok());
        assert!(spirv_entry(&m, "vs", ShaderStage::Fragment).is_err());
        assert!(spirv_entry(&m, "nope", ShaderStage::Compute).is_err());
        assert!(spirv_entry(&[1, 2, 3, 4, 5], "main", ShaderStage::Compute).is_err());
    }
}
