//! Dropping the player into the world: from the main menu (Play, search,
//! map pick) or the pause menu map. A `SpawnRequest` names a place; the
//! ground height is looked up off the main thread, then the world
//! re-anchors on the destination and the player appears in the sky
//! (skydive) or on the ground.

use crate::config::Config;
use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::{EnuFrame, Geodetic};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(Spawner { tx, rx, busy: false, status: String::new() })
            .add_message::<SpawnRequest>()
            .add_systems(Update, (start_requests, finish_requests).chain());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnHeight {
    /// 1,200 m above the ground, skydiving.
    Sky,
    /// Standing on the ground.
    Ground,
}

#[derive(Message, Debug, Clone)]
pub enum SpawnRequest {
    /// A known point (map click, coordinates).
    At { pos: Geodetic, name: String, height: SpawnHeight },
    /// Geocode a typed place name first (search UI is currently not exposed).
    #[allow(dead_code)]
    Search { query: String, height: SpawnHeight },
    /// Somewhere interesting, picked at random (random UI is currently not exposed).
    #[allow(dead_code)]
    Random { height: SpawnHeight },
}

#[derive(Resource)]
pub struct Spawner {
    tx: Sender<Result<(Geodetic, String, SpawnHeight), String>>,
    rx: Receiver<Result<(Geodetic, String, SpawnHeight), String>>,
    /// A request is in flight.
    pub busy: bool,
    /// Last message for the UI ("Finding Zermatt...", errors).
    pub status: String,
}

/// Fired once the player has been placed somewhere new.
#[derive(Resource, Clone)]
pub struct Arrived;

const DROP_AGL_M: f64 = 1_200.0;

/// Hand-picked places that look great from the first second.
pub const PLACES: &[(&str, f64, f64)] = &[
    ("Central Park, New York", 40.7812, -73.9665),
    ("Midtown Manhattan", 40.7549, -73.9840),
    ("Zermatt, Switzerland", 46.0207, 7.7491),
    ("Chamonix, France", 45.9237, 6.8694),
    ("Paris, France", 48.8566, 2.3522),
    ("Montmartre, Paris", 48.8867, 2.3431),
    ("Aix-en-Provence, France", 43.5297, 5.4474),
    ("Gordes, Provence", 43.9116, 5.2004),
    ("Positano, Italy", 40.6281, 14.4850),
    ("Florence, Italy", 43.7696, 11.2558),
    ("Venice, Italy", 45.4408, 12.3155),
    ("Ronda, Spain", 36.7423, -5.1666),
    ("Seville, Spain", 37.3891, -5.9845),
    ("Santorini, Greece", 36.4167, 25.4320),
    ("Marrakech, Morocco", 31.6295, -7.9811),
    ("Chefchaouen, Morocco", 35.1688, -5.2636),
    ("Sahara near Merzouga", 31.0802, -4.0133),
    ("Cairo, Egypt", 30.0444, 31.2357),
    ("Dubai Marina", 25.0805, 55.1403),
    ("Shibuya, Tokyo", 35.6595, 139.7005),
    ("Kyoto, Japan", 35.0116, 135.7681),
    ("Mount Fuji, Japan", 35.3606, 138.7274),
    ("Hong Kong", 22.2793, 114.1628),
    ("Singapore", 1.2834, 103.8607),
    ("Bangkok, Thailand", 13.7563, 100.5018),
    ("Mumbai, India", 19.0760, 72.8777),
    ("Varanasi, India", 25.3176, 82.9739),
    ("Nairobi, Kenya", -1.2921, 36.8219),
    ("Cape Town, South Africa", -33.9249, 18.4241),
    ("Rio de Janeiro, Brazil", -22.9068, -43.1729),
    ("Mexico City", 19.4326, -99.1332),
    ("Cusco, Peru", -13.5320, -71.9675),
    ("San Francisco", 37.7749, -122.4194),
    ("Big Sur, California", 36.2704, -121.8081),
    ("Grand Canyon, Arizona", 36.0544, -112.1401),
    ("Yosemite Valley", 37.7456, -119.5936),
    ("Banff, Canada", 51.1784, -115.5708),
    ("Reykjavik, Iceland", 64.1466, -21.9426),
    ("Lofoten, Norway", 68.2342, 14.5683),
    ("Edinburgh, Scotland", 55.9533, -3.1883),
    ("London, England", 51.5072, -0.1276),
    ("Amsterdam, Netherlands", 52.3676, 4.9041),
    ("Prague, Czechia", 50.0755, 14.4378),
    ("Istanbul, Turkey", 41.0082, 28.9784),
    ("Sydney, Australia", -33.8568, 151.2153),
    ("Uluru, Australia", -25.3444, 131.0369),
    ("Queenstown, New Zealand", -45.0312, 168.6626),
    ("Hallstatt, Austria", 47.5622, 13.6493),
];

fn pick_random(seed: u64) -> (Geodetic, String) {
    let r = ed_world::hash01(seed, 1);
    let (name, lat, lon) = PLACES[((r * PLACES.len() as f32) as usize).min(PLACES.len() - 1)];
    // Jitter a few hundred meters so repeat visits differ.
    let j = |k: u64| (ed_world::hash01(seed, k) as f64 - 0.5) * 0.006;
    (Geodetic::new(lat + j(2), lon + j(3), 0.0), name.to_string())
}

fn start_requests(cfg: Res<Config>, mut sp: ResMut<Spawner>, mut reqs: MessageReader<SpawnRequest>) {
    for req in reqs.read() {
        sp.busy = true;
        let (tx, cache, req) = (sp.tx.clone(), cfg.cache.clone(), req.clone());
        sp.status = match &req {
            SpawnRequest::Search { query, .. } => format!("Finding {query}..."),
            _ => "Loading...".into(),
        };
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let found: Result<(Geodetic, String, SpawnHeight), String> = match req {
                    SpawnRequest::At { pos, name, height } => {
                        let name = if name.is_empty() {
                            ed_data::geocode::reverse(&cache, pos)
                                .ok()
                                .flatten()
                                .filter(|p| !p.name.is_empty())
                                .map(|p| if p.country.is_empty() { p.name } else { format!("{}, {}", p.name, p.country) })
                                .unwrap_or_else(|| format!("{:.4}, {:.4}", pos.lat_deg, pos.lon_deg))
                        } else {
                            name
                        };
                        Ok((pos, name, height))
                    }
                    SpawnRequest::Random { height } => {
                        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
                        let (g, n) = pick_random(seed);
                        Ok((g, n, height))
                    }
                    SpawnRequest::Search { query, height } => match ed_data::geocode::search(&cache, &query) {
                        Ok(r) if !r.is_empty() => Ok((Geodetic { alt_m: 0.0, ..r[0].position }, format!("{}, {}", r[0].name, r[0].country), height)),
                        Ok(_) => Err(format!("Couldn't find \"{query}\". Try a city, town or landmark.")),
                        Err(e) => Err(format!("Search isn't reachable right now ({e}).")),
                    },
                };
                // Ground height at the destination.
                let found = found.map(|(mut g, name, h)| {
                    let t = ed_geo::tile::TileId::containing(g.lat_deg, g.lon_deg, 12);
                    if let Ok(ht) = ed_data::elevation::fetch(&cache, t) {
                        let (u, v) = t.geodetic_to_uv(g.lat_deg, g.lon_deg);
                        g.alt_m = ht.sample(u, v).max(0.0) as f64;
                    }
                    (g, name, h)
                });
                let _ = tx.send(found);
            })
            .detach();
    }
}

#[allow(clippy::type_complexity)]
fn finish_requests(
    mut commands: Commands,
    mut sp: ResMut<Spawner>,
    mut origin: ResMut<WorldOrigin>,
    mut clock: ResMut<crate::sky::Clock>,
    mut gfx: Option<ResMut<crate::menu::GraphicsSettings>>,
    mut players: Query<(
        &mut Transform,
        &mut Player,
        Option<&mut LinearVelocity>,
        Option<&mut AngularVelocity>,
        Option<&mut Position>,
        Option<&mut Rotation>,
    )>,
    mut cams: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<crate::globe::GlobeCamera>)>,
    mut map: Option<ResMut<crate::mapview::MapState>>,
) {
    let Ok(res) = sp.rx.try_recv() else { return };
    sp.busy = false;
    let (dest, name, height) = match res {
        Ok(x) => x,
        Err(msg) => {
            sp.status = msg;
            return;
        }
    };
    // Keep the chosen local solar hour when teleporting (unless the player
    // selected "Now", which tracks the real absolute time). The hour is the
    // one the player set (3 pm by default), not the one where they came from.
    let local_hour = gfx.as_ref().filter(|g| !g.time_live).map(|g| g.solar_hour);
    origin.frame = EnuFrame::new(Geodetic { alt_m: 0.0, ..dest });
    origin.generation += 1;
    if let Some(h) = local_hour {
        clock.unix_s = ed_geo::sun::unix_at_solar_hour(clock.unix_s, dest.lon_deg, h);
    }
    // The clock was set outright, so the pending hour-offset (a map ease
    // that never finished) must not be applied on top of it.
    if let Some(g) = gfx.as_mut() {
        g.hour_offset_applied = g.hour_offset;
    }
    if let Some(m) = map.as_mut() {
        m.time_target_offset = None;
    }
    let agl = match height {
        SpawnHeight::Sky => DROP_AGL_M,
        SpawnHeight::Ground => 2.5,
    };
    let pos = origin.to_render(Geodetic { alt_m: dest.alt_m + agl, ..dest });
    let up = origin.up_at(pos);
    if let Ok((mut t, mut p, vel, ang_vel, ppos, prot)) = players.single_mut() {
        let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw);
        t.translation = pos;
        t.rotation = rot;
        if let Some(mut v) = vel {
            v.0 = Vec3::ZERO;
        }
        if let Some(mut av) = ang_vel {
            av.0 = Vec3::ZERO;
        }
        if let Some(mut pp) = ppos {
            pp.0 = pos;
        }
        if let Some(mut pr) = prot {
            pr.0 = rot;
        }
        p.mode = if height == SpawnHeight::Sky { MoveMode::Skydive } else { MoveMode::Fall };
        p.agl = agl as f32;
        p.grounded = false;
        p.prev_vel = Vec3::ZERO;
        p.accel_smooth = Vec3::ZERO;
    } else if let Ok(mut c) = cams.single_mut() {
        c.translation = pos;
    }
    sp.status = String::new();
    commands.insert_resource(crate::player::Settling { until_s: Some(f32::MAX) });
    commands.insert_resource(crate::hud::ArrivalNotice { name, until_s: 0.0 });
    commands.insert_resource(Arrived);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_places_are_valid() {
        for s in 0..200 {
            let (g, n) = pick_random(s);
            assert!(!n.is_empty());
            assert!(g.lat_deg.abs() < 80.0 && g.lon_deg.abs() <= 180.0);
        }
    }
}
