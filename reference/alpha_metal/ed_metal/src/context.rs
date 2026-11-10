//! Device, queue, and shader-library helpers.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;

pub struct MetalContext {
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
}

impl MetalContext {
    pub fn new() -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        let queue = device.newCommandQueue().ok_or("no command queue")?;
        Ok(MetalContext { device, queue })
    }

    pub fn name(&self) -> String {
        self.device.name().to_string()
    }

    /// Compile MSL source at runtime. Precompiled `.metallib` comes later (build step).
    pub fn library(&self, source: &str) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>, String> {
        self.device
            .newLibraryWithSource_options_error(&NSString::from_str(source), None)
            .map_err(|e| e.localizedDescription().to_string())
    }
}
