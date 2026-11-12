//! W5 atmosphere on the GPU: synthetic ridges at 2, 8 and 30 km (aerial perspective makes the
//! far ones bluer and closer to the sky), a deep-blue zenith over a pale horizon, determinism,
//! and the off switch. With `ED_ATMOS_SHOTS=dir` the captures (1.7 m, 10 km, 400 km at noon and
//! at sun -4 degrees, the ridges, the debug views, atmosphere off) are written there as PNG.
#![cfg(target_os = "macos")]

use ed_geo::ObjectId;
use ed_gpu::*;
use ed_render::scene::*;
use ed_render::*;
use glam::{DVec3, Vec3};

const W: u32 = 1280;
const H: u32 = 720;
const FMT: Format = Format::Rgba8Srgb;

struct Shots {
    dev: Box<dyn Device>,
    r: Option<Renderer>,
    target: TextureId,
    readback: BufferId,
}

impl Shots {
    fn new() -> Option<Self> {
        let dev = device_or_skip(default_backend(), &DeviceDesc { validation: true, frames_in_flight: 2 })?;
        let mut r = Renderer::new(&*dev).expect("renderer");
        r.set_atmosphere(AtmosphereSettings::default());
        let target = dev
            .create_texture(&TextureDesc {
                label: "atmos shot",
                kind: TextureKind::D2,
                format: FMT,
                width: W,
                height: H,
                depth_or_layers: 1,
                mips: 1,
                samples: 1,
                usage: Usage { render_target: true, copy_src: true, ..Default::default() },
            })
            .unwrap();
        let readback = dev
            .create_buffer(&BufferDesc { label: "atmos readback", size: (FMT.copy_row_pitch(W) * H) as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } })
            .unwrap();
        let mut s = Self { dev, r: Some(r), target, readback };
        s.load_world();
        Some(s)
    }

    /// A 400 km ground slab and three ridges straight ahead (-Z) of the origin.
    fn load_world(&mut self) {
        let dev = &*self.dev;
        let r = self.r.as_mut().unwrap();
        let mesh = r.add_mesh(dev, &cube_mesh()).unwrap();
        let node = r.instances().add_node(TestScene::ORIGIN).unwrap();
        let ground = [0.16, 0.17, 0.10];
        let ridge = [0.09, 0.11, 0.07];
        let boxes: [([f32; 3], [f32; 3], [f32; 3]); 4] = [
            ([0.0, -0.25, 0.0], [400_000.0, 0.5, 400_000.0], ground),
            ([-600.0, 30.0, -2_000.0], [1_400.0, 60.0, 200.0], ridge),
            ([1_200.0, 250.0, -8_000.0], [6_000.0, 500.0, 600.0], ridge),
            ([0.0, 1_500.0, -30_000.0], [60_000.0, 3_000.0, 2_000.0], ridge),
        ];
        for (i, (p, s, c)) in boxes.iter().enumerate() {
            let mat = r.add_material(Material::pbr(Vec3::from(*c), 0.0, 0.9)).unwrap();
            let t = glam::Affine3A::from_scale_rotation_translation(Vec3::from(*s), glam::Quat::IDENTITY, Vec3::from(*p));
            r.instances().add(&InstanceDesc { node, transform: t, mesh, material: mat, object: ObjectId(0xA7_0000 + i as u128), flags: 0 }).unwrap();
        }
    }

    /// Renders a few frames (temporal terms settle) and reads the last back.
    fn shot(&mut self, cam: Camera, sun: Vec3) -> Vec<[u8; 4]> {
        let view = View {
            camera: cam,
            viewport: Viewport { x: 0, y: 0, width: W, height: H },
            target: ViewTarget { texture: self.target, format: FMT, width: W, height: H, initial: ResourceState::Undefined, final_state: None },
            flags: Default::default(),
        };
        for i in 0..4 {
            let params = FrameParams { sun_dir: sun, readback: (i == 3).then_some(self.readback), ..Default::default() };
            let r = self.r.as_mut().unwrap();
            r.flush_uploads(&*self.dev).unwrap();
            let mut e = self.dev.begin_frame().unwrap();
            r.render(&*self.dev, &mut *e, &view, &params).unwrap();
            self.dev.end_frame(e).expect("end_frame (validation)");
        }
        self.dev.wait_idle();
        let mut raw = Vec::new();
        self.dev.read_buffer(self.readback, &mut raw).unwrap();
        let pitch = FMT.copy_row_pitch(W) as usize;
        (0..H as usize).flat_map(|y| (0..W as usize).map(move |x| (y, x))).map(|(y, x)| {
            let o = y * pitch + x * 4;
            [raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]
        }).collect()
    }

    fn settings(&mut self, f: impl FnOnce(&mut AtmosphereSettings)) {
        let r = self.r.as_mut().unwrap();
        let mut a = *r.atmosphere();
        f(&mut a);
        r.set_atmosphere(a);
    }

    fn destroy(mut self) {
        self.dev.wait_idle();
        self.r.take().unwrap().destroy(&*self.dev);
        self.dev.destroy_texture(self.target);
        self.dev.destroy_buffer(self.readback);
        self.dev.wait_idle();
    }
}

fn save(px: &[[u8; 4]], name: &str) {
    let Some(dir) = std::env::var_os("ED_ATMOS_SHOTS") else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    let file = std::fs::File::create(&path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), W, H);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut wr = enc.write_header().unwrap();
    let rgba: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    wr.write_image_data(&rgba).unwrap();
    wr.finish().unwrap();
}

fn cam(alt_m: f64, yaw_deg: f32, pitch_deg: f32) -> Camera {
    Camera {
        position: TestScene::ORIGIN + DVec3::new(0.0, alt_m, 0.0),
        yaw: yaw_deg.to_radians(),
        pitch: pitch_deg.to_radians(),
        fov_y: Camera::DEFAULT_FOV_Y,
        near: Camera::DEFAULT_NEAR,
    }
}

/// Unit vector to a sun at `elev` degrees above the horizon, `az` degrees from -Z towards +X.
fn sun(elev: f32, az: f32) -> Vec3 {
    let (e, a) = (elev.to_radians(), az.to_radians());
    Vec3::new(e.cos() * a.sin(), e.sin(), -e.cos() * a.cos())
}

/// Linear (decoded sRGB) RGB averaged over a 5x5 block.
fn lin(px: &[[u8; 4]], x: u32, y: u32) -> Vec3 {
    let mut s = Vec3::ZERO;
    for dy in 0..5 {
        for dx in 0..5 {
            let p = px[((y + dy - 2) * W + x + dx - 2) as usize];
            let d = |c: u8| {
                let c = c as f32 / 255.0;
                if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
            };
            s += Vec3::new(d(p[0]), d(p[1]), d(p[2]));
        }
    }
    s / 25.0
}

/// Pixel of a point relative to the origin node.
fn project(c: &Camera, p: Vec3) -> (u32, u32) {
    let rel = (TestScene::ORIGIN + p.as_dvec3() - c.position).as_vec3();
    let v = c.relative_view_proj(W as f32 / H as f32) * rel.extend(1.0);
    (((v.x / v.w * 0.5 + 0.5) * W as f32) as u32, ((0.5 - v.y / v.w * 0.5) * H as f32) as u32)
}

#[test]
fn ridges_fade_blue_with_distance_and_sky_is_deep_blue_over_pale() {
    let Some(mut s) = Shots::new() else { return };
    // Sun high behind and to the right, so the ridge faces are lit.
    let noon = sun(60.0, 150.0);
    let c = cam(1.7, 0.0, 2.0);
    let px = s.shot(c, noon);
    save(&px, "ridges_noon");

    // The ridge faces, and the sky just above each ridge's top.
    let near_p = project(&c, Vec3::new(-600.0, 30.0, -1_899.0));
    let near = lin(&px, near_p.0, near_p.1);
    let mid_p = project(&c, Vec3::new(1_200.0, 250.0, -7_699.0));
    let mid = lin(&px, mid_p.0, mid_p.1);
    let far_p = project(&c, Vec3::new(-8_000.0, 1_500.0, -28_999.0));
    let far = lin(&px, far_p.0, far_p.1);
    let horizon = lin(&px, far_p.0, project(&c, Vec3::new(-8_000.0, 3_000.0, -29_000.0)).1 - 30);
    let zen = lin(&px, W / 2, 6);
    let luma = |v: Vec3| v.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let blue = |v: Vec3| v.z / v.x.max(1e-4);
    println!("ridge 2 km {near:?} (lum {:.3}, B/R {:.2})", luma(near), blue(near));
    println!("ridge 8 km {mid:?} (lum {:.3}, B/R {:.2})", luma(mid), blue(mid));
    println!("ridge 30 km {far:?} (lum {:.3}, B/R {:.2})", luma(far), blue(far));
    println!("sky above horizon {horizon:?} (lum {:.3}, B/R {:.2}), top of frame {zen:?} (lum {:.3}, B/R {:.2})", luma(horizon), blue(horizon), luma(zen), blue(zen));
    // Bluer with distance.
    assert!(blue(near) < blue(mid) && blue(mid) < blue(far), "ridges should get bluer with distance");
    // Lower contrast with distance: each ridge is closer to the horizon sky's luminance.
    let ch = |v: Vec3| (luma(horizon) - luma(v)).abs() / luma(horizon);
    assert!(ch(near) > ch(mid) && ch(mid) > ch(far), "ridges should lose contrast against the sky with distance");
    // Pale horizon under a bluer, darker-saturated sky higher up.
    assert!(blue(zen) > blue(horizon), "sky higher up should be bluer than at the horizon");

    // Determinism: the same frame twice is byte-identical.
    let again = s.shot(c, noon);
    assert!(px == again, "two renders of the same frame differ");

    // The off switch gives a different (analytic) sky.
    s.settings(|a| a.enabled = false);
    let off = s.shot(c, noon);
    save(&off, "ridges_noon_no_atmos");
    assert!(off != px, "EW_NO_ATMOS path should differ");
    s.destroy();
}

/// Captures for the owner (no assertions beyond "renders"): `ED_ATMOS_SHOTS=dir cargo test -p
/// ed_render --test render_atmos -- --ignored --nocapture`.
#[test]
#[ignore]
fn atmosphere_captures() {
    let Some(mut s) = Shots::new() else { return };
    let noon = sun(60.0, 150.0);
    let dusk = sun(-4.0, 20.0);
    let views = [
        ("alt_1.7m", cam(1.7, 0.0, 10.0)),
        ("alt_10km", cam(10_000.0, 0.0, -8.0)),
        ("alt_400km", cam(400_000.0, 0.0, -20.0)),
    ];
    for (name, c) in views {
        let px = s.shot(c, noon);
        println!("{name} noon: top {:?} middle {:?} bottom {:?}", lin(&px, W / 2, 6), lin(&px, W / 2, H / 2), lin(&px, W / 2, H - 6));
        save(&px, &format!("{name}_noon"));
        let px = s.shot(c, dusk);
        println!("{name} sun -4: top {:?} middle {:?} bottom {:?}", lin(&px, W / 2, 6), lin(&px, W / 2, H / 2), lin(&px, W / 2, H - 6));
        save(&px, &format!("{name}_sun-4"));
    }
    let c = cam(1.7, 0.0, 2.0);
    for (d, name) in [(1, "debug_transmittance"), (2, "debug_inscatter")] {
        s.settings(|a| a.debug = d);
        save(&s.shot(c, noon), &format!("ridges_noon_{name}"));
        save(&s.shot(cam(10_000.0, 0.0, -8.0), noon), &format!("alt_10km_noon_{name}"));
    }
    s.settings(|a| a.debug = 0);
    s.settings(|a| a.enabled = false);
    save(&s.shot(cam(1.7, 0.0, 10.0), noon), "alt_1.7m_noon_no_atmos");
    save(&s.shot(cam(10_000.0, 0.0, -8.0), noon), "alt_10km_noon_no_atmos");
    s.destroy();
}
