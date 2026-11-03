//! Vehicles. `1` spawns a car, `2` a plane next to you; `E` gets in/out.
//!
//! Car: rigid body + four raycast wheels. Each wheel is a spring-damper
//! along the car's up axis; tires produce lateral grip from slip velocity and
//! longitudinal force from the engine/brakes, limited by a friction circle
//! (mu * load). Engine rpm comes from wheel speed through the gearbox.
//!
//! Plane: lift from angle of attack (linear up to stall at ~15°, then drops),
//! induced + parasitic drag, thrust, and control torques that scale with
//! dynamic pressure, so it flies like a plane, not a cursor.

use crate::origin::{Focus, Rebased, WorldOrigin};
use crate::player::Player;
use avian3d::prelude::*;
use bevy::prelude::*;
use ed_flight::atmosphere;

pub struct VehiclesPlugin;

impl Plugin for VehiclesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Driving>()
            .add_systems(Update, (spawn_vehicles, enter_exit, vehicle_camera))
            .add_systems(FixedUpdate, (car_physics, plane_physics));
    }
}

#[derive(Resource, Default)]
pub struct Driving(pub Option<Entity>);

#[derive(Component)]
pub struct Car {
    pub rpm: f32,
    pub gear: usize,
    pub steer: f32,
    pub throttle: f32,
    pub brake: f32,
    pub compression: [f32; 4],
}

#[derive(Component)]
pub struct Plane {
    pub throttle: f32,
    pub pitch: f32,
    pub roll: f32,
    pub yaw: f32,
}

const WHEELS: [Vec3; 4] = [
    Vec3::new(-0.8, -0.3, -1.35), // front left (-Z is forward)
    Vec3::new(0.8, -0.3, -1.35),
    Vec3::new(-0.8, -0.3, 1.35),
    Vec3::new(0.8, -0.3, 1.35),
];
const REST: f32 = 0.45;
const WHEEL_R: f32 = 0.34;
const GEARS: [f32; 6] = [3.6, 2.2, 1.5, 1.15, 0.92, 0.76];
const FINAL_DRIVE: f32 = 3.7;

fn spawn_vehicles(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    players: Query<(&Transform, &Player)>,
    origin: Res<WorldOrigin>,
) {
    let Ok((pt, p)) = players.single() else { return };
    let up = origin.up_at(pt.translation);
    let base = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw);
    let ahead = pt.translation + base * Vec3::new(0.0, 1.5, -6.0);
    if keys.just_pressed(KeyCode::Digit1) {
        let paint = mats.add(StandardMaterial { base_color: Color::srgb(0.95, 0.75, 0.82), perceptual_roughness: 0.3, clearcoat: 1.0, ..default() });
        let dark = mats.add(StandardMaterial { base_color: Color::srgb(0.1, 0.1, 0.12), perceptual_roughness: 0.5, ..default() });
        commands
            .spawn((
                Car { rpm: 900.0, gear: 0, steer: 0.0, throttle: 0.0, brake: 0.0, compression: [0.0; 4] },
                RigidBody::Dynamic,
                Collider::cuboid(1.8, 0.9, 4.2),
                Mass(1_400.0),
                CenterOfMass(Vec3::new(0.0, -0.35, 0.0)),
                Transform::from_translation(ahead).with_rotation(base),
                Mesh3d(meshes.add(Cuboid::new(1.8, 0.9, 4.2))),
                MeshMaterial3d(paint.clone()),
                Rebased,
                Name::new("car"),
            ))
            .with_children(|c| {
                c.spawn((Mesh3d(meshes.add(Cuboid::new(1.6, 0.6, 2.0))), MeshMaterial3d(paint), Transform::from_xyz(0.0, 0.7, 0.3)));
                for w in WHEELS {
                    c.spawn((
                        Mesh3d(meshes.add(Cylinder::new(WHEEL_R, 0.25))),
                        MeshMaterial3d(dark.clone()),
                        Transform::from_translation(w - Vec3::Y * 0.25).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ));
                }
            });
    }
    if keys.just_pressed(KeyCode::Digit2) {
        let white = mats.add(StandardMaterial { base_color: Color::srgb(0.95, 0.95, 0.97), perceptual_roughness: 0.35, ..default() });
        let accent = mats.add(StandardMaterial { base_color: Color::srgb(0.98, 0.45, 0.1), perceptual_roughness: 0.5, ..default() });
        commands
            .spawn((
                Plane { throttle: 0.0, pitch: 0.0, roll: 0.0, yaw: 0.0 },
                RigidBody::Dynamic,
                Collider::cuboid(1.2, 1.2, 8.0),
                Mass(1_100.0),
                Transform::from_translation(ahead + up * 1.0).with_rotation(base),
                Mesh3d(meshes.add(Capsule3d::new(0.6, 7.0))),
                MeshMaterial3d(white.clone()),
                Rebased,
                Name::new("plane"),
            ))
            .with_children(|c| {
                // Capsule is Y-up; lay the fuselage along -Z.
                c.spawn((Mesh3d(meshes.add(Cuboid::new(11.0, 0.15, 1.6))), MeshMaterial3d(white.clone()), Transform::from_xyz(0.0, 0.0, -0.3)));
                c.spawn((Mesh3d(meshes.add(Cuboid::new(3.6, 0.12, 1.0))), MeshMaterial3d(white.clone()), Transform::from_xyz(0.0, 0.3, 3.6)));
                c.spawn((Mesh3d(meshes.add(Cuboid::new(0.12, 1.5, 1.2))), MeshMaterial3d(accent), Transform::from_xyz(0.0, 0.9, 3.7)));
            })
            .insert(Transform::from_translation(ahead + up * 1.0).with_rotation(base));
    }
}

fn enter_exit(
    keys: Res<ButtonInput<KeyCode>>,
    mut driving: ResMut<Driving>,
    mut commands: Commands,
    mut players: Query<(Entity, &mut Transform, &mut Visibility), (With<Player>, Without<Car>, Without<Plane>)>,
    vehicles: Query<(Entity, &Transform), Or<(With<Car>, With<Plane>)>>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok((pe, mut pt, mut vis)) = players.single_mut() else { return };
    match driving.0 {
        Some(v) => {
            if let Ok((_, vt)) = vehicles.get(v) {
                pt.translation = vt.translation + vt.right() * 2.5 + vt.up() * 1.0;
            }
            commands.entity(pe).remove::<(RigidBodyDisabled, ColliderDisabled)>().insert(Focus);
            commands.entity(v).remove::<Focus>();
            *vis = Visibility::Inherited;
            driving.0 = None;
        }
        None => {
            let near = vehicles.iter().min_by(|a, b| a.1.translation.distance(pt.translation).total_cmp(&b.1.translation.distance(pt.translation)));
            if let Some((v, vt)) = near.filter(|(_, vt)| vt.translation.distance(pt.translation) < 8.0) {
                let _ = vt;
                commands.entity(pe).insert((RigidBodyDisabled, ColliderDisabled)).remove::<Focus>();
                commands.entity(v).insert(Focus);
                *vis = Visibility::Hidden;
                driving.0 = Some(v);
            }
        }
    }
}

fn car_physics(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    driving: Res<Driving>,
    spatial: SpatialQuery,
    mut cars: Query<(Entity, &Transform, &LinearVelocity, &AngularVelocity, &mut Car, Forces)>,
) {
    let dt = time.delta_secs();
    for (e, t, lv, av, mut car, mut forces) in &mut cars {
        let active = driving.0 == Some(e);
        let (thr, brk, steer_in) = if active {
            let f = keys.pressed(KeyCode::KeyW) as i32 as f32;
            let b = keys.pressed(KeyCode::KeyS) as i32 as f32;
            let s = keys.pressed(KeyCode::KeyA) as i32 as f32 - keys.pressed(KeyCode::KeyD) as i32 as f32;
            (f, b, s)
        } else {
            (0.0, 0.3, 0.0)
        };
        let fwd = t.forward().as_vec3();
        let up = t.up().as_vec3();
        let right = t.right().as_vec3();
        let speed_fwd = lv.0.dot(fwd);
        // Steering eases off with speed so it stays drivable at 200 km/h.
        let max_steer = 0.6 / (1.0 + speed_fwd.abs() / 18.0);
        car.steer += (steer_in * max_steer - car.steer) * (1.0 - (-8.0 * dt).exp());
        car.throttle = thr;
        car.brake = brk;
        // Engine & gearbox (automatic).
        let wheel_rps = speed_fwd.abs() / (std::f32::consts::TAU * WHEEL_R);
        car.rpm = (wheel_rps * 60.0 * GEARS[car.gear] * FINAL_DRIVE).max(850.0);
        if car.rpm > 6_300.0 && car.gear < GEARS.len() - 1 {
            car.gear += 1;
        } else if car.rpm < 2_200.0 && car.gear > 0 {
            car.gear -= 1;
        }
        // Torque curve: peaky around 4500 rpm.
        let x = (car.rpm - 4_500.0) / 3_500.0;
        let torque = 420.0 * (1.0 - 0.6 * x * x).max(0.2);
        let drive_force = torque * GEARS[car.gear] * FINAL_DRIVE / WHEEL_R * thr;
        let filter = SpatialQueryFilter::from_excluded_entities([e]);
        let com = t.translation;
        for (i, w) in WHEELS.iter().enumerate() {
            let mount = t.transform_point(*w);
            let Ok(dir) = Dir3::new(-up) else { continue };
            let Some(hit) = spatial.cast_ray(mount, dir, REST + WHEEL_R, true, &filter) else {
                car.compression[i] = 0.0;
                continue;
            };
            let comp = REST + WHEEL_R - hit.distance;
            let prev = car.compression[i];
            car.compression[i] = comp;
            let comp_vel = (comp - prev) / dt.max(1e-4);
            let spring = (comp * 45_000.0 + comp_vel * 4_000.0).max(0.0);
            let contact = mount - up * (hit.distance - 0.0);
            forces.apply_force_at_point(up * spring, contact);
            // Tire forces in the wheel's frame.
            let steer = if i < 2 { car.steer } else { 0.0 };
            let wfwd = Quat::from_axis_angle(up, steer) * fwd;
            let wright = wfwd.cross(up);
            let v_point = lv.0 + av.0.cross(contact - com);
            let v_lat = v_point.dot(wright);
            let v_long = v_point.dot(wfwd);
            let grip = 1.1 * spring; // mu * load
            let mut f_lat = -v_lat * 1_400.0;
            let mut f_long = if i >= 2 { drive_force * 0.5 } else { 0.0 };
            f_long -= v_long.signum() * brk * 0.9 * spring;
            f_long -= v_long * 8.0; // rolling resistance
            // Friction circle.
            let total = (f_lat * f_lat + f_long * f_long).sqrt();
            if total > grip {
                let k = grip / total;
                f_lat *= k;
                f_long *= k;
            }
            forces.apply_force_at_point(wright * f_lat + wfwd * f_long, contact);
        }
        // Aero drag + downforce.
        let v2 = lv.0.length_squared();
        forces.apply_force(-lv.0.normalize_or_zero() * 0.5 * 1.2 * 0.7 * v2 - up * 0.3 * v2);
        let _ = right;
    }
}

fn plane_physics(
    keys: Res<ButtonInput<KeyCode>>,
    driving: Res<Driving>,
    origin: Res<WorldOrigin>,
    mut planes: Query<(Entity, &Transform, &LinearVelocity, &AngularVelocity, &mut Plane, Forces)>,
) {
    for (e, t, lv, av, mut p, mut forces) in &mut planes {
        if driving.0 == Some(e) {
            if keys.pressed(KeyCode::ShiftLeft) {
                p.throttle = (p.throttle + 0.01).min(1.0);
            }
            if keys.pressed(KeyCode::ControlLeft) {
                p.throttle = (p.throttle - 0.01).max(0.0);
            }
            p.pitch = keys.pressed(KeyCode::KeyS) as i32 as f32 - keys.pressed(KeyCode::KeyW) as i32 as f32;
            p.roll = keys.pressed(KeyCode::KeyA) as i32 as f32 - keys.pressed(KeyCode::KeyD) as i32 as f32;
            p.yaw = keys.pressed(KeyCode::KeyQ) as i32 as f32 - keys.pressed(KeyCode::KeyE) as i32 as f32;
        }
        let alt = origin.to_geodetic(t.translation).alt_m;
        let rho = atmosphere(alt).density as f32;
        let fwd = t.forward().as_vec3();
        let up = t.up().as_vec3();
        let right = t.right().as_vec3();
        let v = lv.0;
        let speed = v.length();
        let q = 0.5 * rho * speed * speed;
        // Angle of attack in the pitch plane.
        let v_body_fwd = v.dot(fwd);
        let v_body_up = v.dot(up);
        let aoa = (-v_body_up).atan2(v_body_fwd.max(0.1));
        let cl = if aoa.abs() < 0.26 { 0.25 + 5.0 * aoa } else { (0.25 + 5.0 * 0.26 * aoa.signum()) * 0.5 };
        let wing_area = 16.0;
        let lift_dir = v.cross(right).normalize_or_zero();
        let lift = lift_dir * (q * wing_area * cl);
        let cd = 0.03 + 0.05 * cl * cl;
        let drag = -v.normalize_or_zero() * q * wing_area * cd;
        let thrust = fwd * p.throttle * 9_000.0;
        forces.apply_force(lift + drag + thrust);
        // Control torques scale with dynamic pressure; damping keeps it stable.
        let ctrl = q * 0.9;
        let torque = right * (p.pitch * ctrl * 2.0) + fwd * (-p.roll * ctrl * 3.0) + up * (p.yaw * ctrl * 1.0) - av.0 * (q * 2.5 + 400.0);
        forces.apply_torque(torque);
    }
}

fn vehicle_camera(
    driving: Res<Driving>,
    vehicles: Query<&Transform, (Or<(With<Car>, With<Plane>)>, Without<Camera3d>)>,
    mut cams: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    let Some(v) = driving.0 else { return };
    let (Ok(vt), Ok(mut ct)) = (vehicles.get(v), cams.single_mut()) else { return };
    let target = vt.translation - vt.forward().as_vec3() * 9.0 + vt.up().as_vec3() * 3.0;
    let k = 1.0 - (-6.0 * time.delta_secs()).exp();
    ct.translation = ct.translation.lerp(target, k);
    let look = Transform::from_translation(ct.translation).looking_at(vt.translation + vt.up().as_vec3() * 1.0, vt.up().as_vec3());
    ct.rotation = ct.rotation.slerp(look.rotation, k);
}
