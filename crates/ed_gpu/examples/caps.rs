//! Prints the capabilities of a backend on this machine: `ED_GPU_BACKEND` when set, otherwise
//! the native one.
//!
//! `cargo run -p ed_gpu --example caps`
//! `ED_GPU_BACKEND=vulkan cargo run -p ed_gpu --features vulkan --example caps`

use ed_gpu::{DeviceDesc, create_device, default_backend};

fn main() {
    let backend = default_backend();
    match create_device(backend, &DeviceDesc { validation: false, frames_in_flight: 2 }) {
        Ok(dev) => println!("{:#?}", dev.caps()),
        Err(e) => {
            eprintln!("{backend:?}: no device: {e}");
            std::process::exit(1);
        }
    }
}
