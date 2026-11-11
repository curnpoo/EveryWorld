//! Prints the capabilities of the native backend on this machine.
//!
//! `cargo run -p ed_gpu --example caps`

use ed_gpu::{Backend, DeviceDesc, create_device};

fn main() {
    let backend = if cfg!(target_os = "macos") {
        Backend::Metal
    } else if cfg!(windows) {
        Backend::D3D12
    } else {
        Backend::Vulkan
    };
    match create_device(backend, &DeviceDesc { validation: false, frames_in_flight: 2 }) {
        Ok(dev) => println!("{:#?}", dev.caps()),
        Err(e) => {
            eprintln!("{backend:?}: no device: {e}");
            std::process::exit(1);
        }
    }
}
