//! Renders the sky for several sun elevations from the atmosphere LUTs and times the LUT passes.
//! `cargo run --release -p ed_metal --bin atmo_probe -- [out_dir]`
#[cfg(not(target_os = "macos"))]
fn main() {}
#[cfg(target_os = "macos")]
use ed_metal::MetalContext;
#[cfg(target_os = "macos")]
#[path = "../atmosphere.rs"]
mod atmosphere;

#[cfg(target_os = "macos")]
fn main() {
    use atmosphere::*;
    use glam::{Mat4, Vec3};
    use objc2::runtime::ProtocolObject;
    use objc2_foundation::NSString;
    use objc2_metal::*;
    use std::ffi::c_void;
    use std::ptr::NonNull;

    let out_dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/claude-501/atmo/".into());
    let _ = std::fs::create_dir_all(&out_dir);
    let ctx = MetalContext::new().expect("metal");
    let atmo = Atmosphere::new(&ctx).expect("atmosphere");
    let (w, h) = (1280usize, 720usize);

    let probe_src = format!(
        "{}\n{}",
        MSL_SKY_FUNCS,
        r#"
struct PU { float4 sun; float4 illum; float4x4 inv_vp; float4 cam; };
kernel void probe(device uchar4* out [[buffer(0)]], constant PU& U [[buffer(1)]],
                  texture2d<float> sky [[texture(0)]], texture2d<float> trans [[texture(1)]], texture3d<float> ap [[texture(2)]],
                  uint2 gid [[thread_position_in_grid]]) {
    const uint W = 1280, H = 720;
    if (gid.x >= W || gid.y >= H) return;
    float2 uv = (float2(gid) + 0.5) / float2(W, H);
    float2 ndc = float2(uv.x * 2 - 1, 1 - uv.y * 2);
    float4 a = U.inv_vp * float4(ndc, 1, 1), b = U.inv_vp * float4(ndc, 0.5, 1);
    float3 dir = normalize(b.xyz / b.w - a.xyz / a.w);
    float3 sun = U.sun.xyz; float hm = U.cam.x;
    float3 col;
    float t = dir.y < -1e-4 ? hm / -dir.y : 1e9;
    if (t < 1e8) {
        // flat grey-green ground, sun + sky ambient, with aerial perspective
        float3 sunT = trans.sample(AT_SMP, at_trans_uv(AT_BOT + 0.001, sun.y), level(0)).rgb;
        float3 amb = sky_radiance(float3(0,1,0), hm, sun, sky) * 0.3;
        float3 L = float3(0.25, 0.3, 0.2) / AT_PI * (U.illum.rgb * sunT * max(sun.y, 0.0) + amb * AT_PI);
        float4 apv = aerial_perspective(uv, t, ap);
        col = L * apv.a + apv.rgb;
    } else {
        col = sky_radiance(dir, hm, sun, sky) + sun_disk(dir, sun, U.illum.rgb, hm, trans);
    }
    col = col / (1.0 + col);
    col = pow(col, float3(1.0 / 2.2));
    out[gid.y * W + gid.x] = uchar4(uchar3(saturate(col) * 255.0 + 0.5), 255);
}
"#
    );
    let lib = ctx.library(&probe_src).expect("probe shader");
    let f = lib.newFunctionWithName(&NSString::from_str("probe")).unwrap();
    let pso = ctx.device.newComputePipelineStateWithFunction_error(&f).unwrap();
    let buf = ctx.device.newBufferWithLength_options(w * h * 4, MTLResourceOptions::StorageModeShared).unwrap();

    #[repr(C)]
    struct PU {
        sun: [f32; 4],
        illum: [f32; 4],
        inv_vp: [[f32; 4]; 4],
        cam: [f32; 4],
    }

    let cam_h = 100.0f32;
    let proj = Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), w as f32 / h as f32, 0.1);
    let view = Mat4::look_to_rh(Vec3::new(0.0, cam_h, 0.0), Vec3::new(0.9, 0.2, 0.0).normalize(), Vec3::Y);
    let inv = (proj * view).inverse();
    let illum = [20.0f32, 20.0, 20.0];

    let time_cb = |name: &str, f: &dyn Fn(&ProtocolObject<dyn MTLCommandBuffer>)| {
        let n = 20;
        let mut tot = 0.0;
        for i in 0..n + 2 {
            let cb = ctx.queue.commandBuffer().unwrap();
            f(&cb);
            cb.commit();
            cb.waitUntilCompleted();
            if i >= 2 {
                tot += unsafe { cb.GPUEndTime() - cb.GPUStartTime() };
            }
        }
        println!("{name:>14}: {:.3} ms", tot / n as f64 * 1000.0);
    };

    for elev in [60.0f32, 10.0, 2.0, -5.0] {
        let e = elev.to_radians();
        let p = AtmosphereParams {
            sun_dir: [e.cos(), e.sin(), 0.0],
            sun_illuminance: illum,
            cam_height_m: cam_h,
            view_proj_inv: inv.to_cols_array_2d(),
            cam_pos: [0.0, cam_h, 0.0],
        };
        if elev == 60.0 {
            time_cb("transmittance", &|cb| atmo.encode_transmittance(cb));
            time_cb("multiscatter", &|cb| atmo.encode_multiscatter(cb));
            time_cb("sky_view", &|cb| atmo.encode_sky_view(cb, &p));
            time_cb("aerial", &|cb| atmo.encode_aerial(cb, &p));
        }
        let cb = ctx.queue.commandBuffer().unwrap();
        atmo.encode_luts(&cb, &p);
        let enc = cb.computeCommandEncoder().unwrap();
        enc.setComputePipelineState(&pso);
        let pu = PU { sun: [p.sun_dir[0], p.sun_dir[1], p.sun_dir[2], 0.0], illum: [illum[0], illum[1], illum[2], 0.0], inv_vp: p.view_proj_inv, cam: [cam_h, 0.0, 0.0, 0.0] };
        unsafe {
            enc.setBuffer_offset_atIndex(Some(&buf), 0, 0);
            enc.setBytes_length_atIndex(NonNull::new(&pu as *const PU as *mut c_void).unwrap(), std::mem::size_of::<PU>(), 1);
            enc.setTexture_atIndex(Some(atmo.sky_view()), 0);
            enc.setTexture_atIndex(Some(atmo.transmittance()), 1);
            enc.setTexture_atIndex(Some(atmo.aerial()), 2);
        }
        enc.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: w.div_ceil(8), height: h.div_ceil(8), depth: 1 }, MTLSize { width: 8, height: 8, depth: 1 });
        enc.endEncoding();
        cb.commit();
        cb.waitUntilCompleted();

        let px = unsafe { std::slice::from_raw_parts(buf.contents().as_ptr() as *const u8, w * h * 4) };
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for c in px.chunks_exact(4) {
            ppm.extend_from_slice(&c[..3]);
        }
        let path = format!("{}/sky_{}.ppm", out_dir.trim_end_matches('/'), elev as i32);
        std::fs::write(&path, ppm).unwrap();
        println!("wrote {path}");
    }
}
