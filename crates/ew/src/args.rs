//! Command line (hand-rolled; no dependency).

use crate::camera::PathKind;
use std::path::PathBuf;

pub const USAGE: &str = "\
ew: EveryWorld engine shell (Phase 1, M4)

USAGE:
  ew [OPTIONS]                 open a window in the proving ground (fly camera)
  ew --headless [OPTIONS]      render offscreen, no window, write PNG + CSV + summary
  ew --play zermatt|blueridge|grasslab  walk on real terrain (window, third-person character; tiles
                               from EARTHDROP_CACHE, an absolute path, offline)

OPTIONS:
  --width W, --height H        output size in pixels (default 1280x720)
  --frames N                   stop after N frames (windowed: presented frames; 0 = run
                               until closed; headless default 300)
  --vsync on|off               windowed: vsync on (default) or off (uncapped). V toggles it
  --headless                   offscreen harness (P1-D15)
  --out DIR                    headless output directory (default target/headless)
  --path orbit|static|proving  headless camera path (default orbit; proving implies
                               --scene proving)
  --play zermatt|blueridge|grasslab     windowed: walk there (W A S D walk, Shift run, Space jump,
                               mouse looks, wheel zooms, V vsync, Esc quits).
                               EW_AUTOWALK=S walks S seconds by itself, writes
                               play_start.png / play_end.png to --out and exits
  --character robot|solarcat   --play: the player character (default robot: the alpha's
                               procedural robot; solarcat: the authored skinned character).
                               C puts the car 4 m ahead of the robot
  --path zermatt|blueridge|grasslab     headless real terrain from EARTHDROP_CACHE (offline; no
                               scene loaded): ground level, then a climb
  --scene test|proving         what to load (windowed default proving; headless default
                               test, or proving with --path proving)
  --speed X                    camera path speed multiplier (default 1)
  --lod-px PX                  LOD threshold, projected error in pixels (default 1)
  --lod-fade N                 LOD cross-fade length in frames, 0 = hard switch (default 16)
  --no-occlusion               frustum culling only (no depth-pyramid occlusion test)
  --pop                        headless: run the pop detector (per-tile luminance change)
  --dump-vis                   headless: write the last frame's visibility buffer to OUT/vis.bin (backend parity)
  --empty                      clear + tonemap only (the empty-frame budget)
  --packs DIR                  pack folder (default target/packs): proving.ewpack (required),
                               bench.ewpack and character.ewpack (optional)
  --pack FILE                  mount one more .ewpack (repeatable), after the built-in packs;
                               EW_PACKS=a.ewpack:b.ewpack does the same. A missing one warns.
                               With a Megascans pack (assets-src/megascans.p1.toml) the proving
                               ground draws real textured materials (ew::real_materials)
  --camera X,Y,Z,YAW,PITCH     headless: fixed camera (metres from the proving origin, y up;
                               radians) instead of the path; the path still drives time
  --ground LAT,LON,HEADING,PITCH,AGL
                               headless terrain paths: fixed eye AGL metres above the
                               resident ground at LAT,LON (degrees), looking along HEADING
                               (degrees from north, clockwise) and PITCH (degrees, + up)
  --sun AZ,EL                  headless: sun azimuth (degrees from north, clockwise) and
                               elevation (degrees) in the scene's local frame
  --date YYYY-MM-DD            tree seasons: the calendar date (default midsummer; EW_SEASON=0
                               turns seasons off, EW_SEASON_RATE=r scales the year about midsummer)
  --layers DIR                 layer root with authored placements (default assets-src/layers)
  --mode game|editor           start mode (default editor; Tab or F1 switches). Headless game
                               mode walks the character along the scripted route
  --editor-ui                  headless: paint the editor panels over the frame
  --inset                      second editor viewport (top-down inset, P1-D24)
  --collision-debug            draw the collision wireframe (C toggles it windowed)
  --bare                       headless: no sim, layers or editor (the M3 benchmark)
  --sim-route                  run the scripted character route without a GPU, print hashes
  --validation                 GPU validation on (default in debug builds)
  --frames-in-flight 2|3       default 2
  -h, --help

KEYS (windowed):
  Tab / F1 switch game <-> editor. Game: W A S D run, Shift sprint, Space jump, mouse
  looks (hold left mouse). Editor: W A S D Q E fly, hold right mouse to look, left click
  picks, gizmo drags, Cmd+S saves layers, Cmd+Z undo. C collision wireframe, I inset view,
  V vsync, Esc quit (game) or clear selection (editor)";

/// What the renderer is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    /// The six-box test scene.
    Test,
    /// The 100k-instance city ([`crate::proving`]).
    Proving,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub headless: bool,
    pub width: u32,
    pub height: u32,
    pub frames: Option<u64>,
    pub vsync: bool,
    pub out: PathBuf,
    pub path: PathKind,
    pub empty: bool,
    pub validation: bool,
    pub frames_in_flight: u32,
    /// `None`: the default for the mode (see [`Args::scene`]).
    pub scene: Option<SceneKind>,
    pub speed: f64,
    pub lod_px: Option<f32>,
    pub lod_fade: Option<u32>,
    pub no_occlusion: bool,
    pub pop: bool,
    /// Headless: write the last frame's visibility buffer (raw u32 pairs) to `out/vis.bin`.
    pub dump_vis: bool,
    /// Pack folder ([`crate::content`]).
    pub packs: PathBuf,
    /// Extra packs (`--pack`, repeatable), mounted after the built-in ones.
    pub extra_packs: Vec<PathBuf>,
    /// Fixed headless camera: x, y, z (from the proving origin), yaw, pitch.
    pub camera: Option<[f64; 5]>,
    /// Fixed near-ground terrain eye: lat, lon, heading, pitch (degrees), height above ground (m).
    pub ground: Option<[f64; 5]>,
    /// Sun azimuth and elevation (degrees, local frame).
    pub sun: Option<[f64; 2]>,
    /// Calendar date (year, month, day) the trees' seasons show (`--date`); default midsummer.
    pub date: Option<(i32, u32, u32)>,
    /// Layer root (authored placements, `assets-src/layers`).
    pub layers: PathBuf,
    /// Run the scripted character route without a GPU and print its hashes.
    pub sim_route: bool,
    /// Start in this mode (windowed; headless: the frame mode, see `--mode`).
    pub mode: crate::session::Mode,
    /// Headless: draw the editor UI (panels) over the frame.
    pub editor_ui: bool,
    /// Second (top-down inset) editor viewport.
    pub inset: bool,
    /// Collision debug draw (wireframe).
    pub collision_debug: bool,
    /// No sim, no layers, no editor (the M3 benchmark exactly).
    pub bare: bool,
    /// Walk on real terrain at this place (windowed; [`crate::play`]).
    pub play: Option<PathKind>,
    /// `--play`: which character walks (default the robot).
    pub character: Character,
}

/// The player character in `--play`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Character {
    /// The alpha's procedural robot: floating-capsule controller, procedural gait.
    #[default]
    Robot,
    /// The authored skinned character on the mover (the proving-ground controller).
    Solarcat,
}

impl Args {
    /// The scene to load: explicit `--scene`, else proving for windowed runs and for
    /// `--path proving`, else the test scene.
    /// Day of year of `--date`, if given.
    pub fn season_day(&self) -> Option<f64> {
        self.date.map(|(y, m, d)| ed_world::season::day_of_year(y, m, d))
    }

    pub fn scene(&self) -> SceneKind {
        self.scene.unwrap_or(if !self.headless || self.path == PathKind::Proving { SceneKind::Proving } else { SceneKind::Test })
    }

    /// Applies the LOD flags to a provider config.
    pub fn apply_lod(&self, c: &mut ed_render::provider::LodConfig) {
        if let Some(px) = self.lod_px {
            c.lod_px = px;
        }
        if let Some(f) = self.lod_fade {
            c.fade_frames = f;
        }
        if self.no_occlusion {
            c.occlusion = false;
        }
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            headless: false,
            width: 1280,
            height: 720,
            frames: None,
            vsync: true,
            out: PathBuf::from("target/headless"),
            path: PathKind::Orbit,
            empty: false,
            validation: cfg!(debug_assertions),
            frames_in_flight: 2,
            scene: None,
            speed: 1.0,
            lod_px: None,
            lod_fade: None,
            no_occlusion: false,
            pop: false,
            dump_vis: false,
            packs: crate::content::default_pack_dir(),
            extra_packs: Vec::new(),
            camera: None,
            ground: None,
            sun: None,
            date: None,
            layers: crate::authored::default_layer_root(),
            sim_route: false,
            mode: crate::session::Mode::Editor,
            editor_ui: false,
            inset: false,
            collision_debug: false,
            bare: false,
            play: None,
            character: Character::default(),
        }
    }
}

/// `Ok(None)` means `--help`.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    fn num<T: std::str::FromStr>(flag: &str, v: Option<String>) -> Result<T, String> {
        let v = v.ok_or_else(|| format!("{flag} needs a value"))?;
        v.parse().map_err(|_| format!("{flag}: bad value {v:?}"))
    }
    fn nums(flag: &str, v: Option<String>, n: usize) -> Result<Vec<f64>, String> {
        let v = v.ok_or_else(|| format!("{flag} needs {n} comma-separated numbers"))?;
        let x: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("{flag}: bad value {v:?}"))?;
        if x.len() != n || x.iter().any(|x| !x.is_finite()) {
            return Err(format!("{flag}: expected {n} numbers, got {v:?}"));
        }
        Ok(x)
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--headless" => a.headless = true,
            "--empty" => a.empty = true,
            "--validation" => a.validation = true,
            "--no-occlusion" => a.no_occlusion = true,
            "--pop" => a.pop = true,
            "--dump-vis" => a.dump_vis = true,
            "--sim-route" => a.sim_route = true,
            "--editor-ui" => a.editor_ui = true,
            "--inset" => a.inset = true,
            "--collision-debug" => a.collision_debug = true,
            "--bare" => a.bare = true,
            "--layers" => a.layers = PathBuf::from(it.next().ok_or("--layers needs a value")?),
            "--mode" => {
                a.mode = match it.next().as_deref() {
                    Some("game") => crate::session::Mode::Game,
                    Some("editor") => crate::session::Mode::Editor,
                    v => return Err(format!("--mode: expected game|editor, got {v:?}")),
                }
            }
            "--speed" => a.speed = num(&arg, it.next())?,
            "--lod-px" => a.lod_px = Some(num(&arg, it.next())?),
            "--lod-fade" => {
                let f: u32 = num(&arg, it.next())?;
                if f > 63 {
                    return Err("--lod-fade: at most 63 frames".into());
                }
                a.lod_fade = Some(f);
            }
            "--scene" => {
                a.scene = Some(match it.next().as_deref() {
                    Some("test") => SceneKind::Test,
                    Some("proving") => SceneKind::Proving,
                    v => return Err(format!("--scene: expected test|proving, got {v:?}")),
                })
            }
            "--width" => a.width = num(&arg, it.next())?,
            "--height" => a.height = num(&arg, it.next())?,
            "--frames" => a.frames = Some(num(&arg, it.next())?),
            "--frames-in-flight" => a.frames_in_flight = num(&arg, it.next())?,
            "--packs" => a.packs = PathBuf::from(it.next().ok_or("--packs needs a value")?),
            "--pack" => a.extra_packs.push(PathBuf::from(it.next().ok_or("--pack needs a value")?)),
            "--camera" => {
                let v = it.next().ok_or("--camera needs X,Y,Z,YAW,PITCH")?;
                let n: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("--camera: bad value {v:?}"))?;
                if n.len() != 5 || n.iter().any(|x| !x.is_finite()) {
                    return Err(format!("--camera: expected 5 numbers X,Y,Z,YAW,PITCH, got {v:?}"));
                }
                a.camera = Some([n[0], n[1], n[2], n[3], n[4]]);
            }
            "--ground" => {
                let n = nums("--ground", it.next(), 5)?;
                a.ground = Some([n[0], n[1], n[2], n[3], n[4]]);
            }
            "--sun" => {
                let n = nums("--sun", it.next(), 2)?;
                a.sun = Some([n[0], n[1]]);
            }
            "--date" => {
                let v = it.next().ok_or("--date needs YYYY-MM-DD")?;
                let bad = || format!("--date: expected YYYY-MM-DD, got {v:?}");
                let mut f = v.split('-');
                let (y, m, d) = (f.next().and_then(|x| x.parse::<i32>().ok()), f.next().and_then(|x| x.parse::<u32>().ok()), f.next().and_then(|x| x.parse::<u32>().ok()));
                match (y, m, d, f.next()) {
                    (Some(y), Some(m), Some(d), None) if (1..=12).contains(&m) && (1..=31).contains(&d) => a.date = Some((y, m, d)),
                    _ => return Err(bad()),
                }
            }
            "--out" => a.out = PathBuf::from(it.next().ok_or("--out needs a value")?),
            "--vsync" => {
                a.vsync = match it.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    v => return Err(format!("--vsync: expected on|off, got {v:?}")),
                }
            }
            "--play" => {
                a.play = Some(match it.next().as_deref() {
                    Some("zermatt") => PathKind::Zermatt,
                    Some("blueridge") => PathKind::BlueRidge,
                    Some("grasslab") => PathKind::GrassLab,
                    v => return Err(format!("--play: expected zermatt|blueridge|grasslab, got {v:?}")),
                })
            }
            "--character" => {
                a.character = match it.next().as_deref() {
                    Some("robot") => Character::Robot,
                    Some("solarcat") => Character::Solarcat,
                    v => return Err(format!("--character: expected robot|solarcat, got {v:?}")),
                }
            }
            "--path" => {
                a.path = match it.next().as_deref() {
                    Some("orbit") => PathKind::Orbit,
                    Some("static") => PathKind::Static,
                    Some("proving") => PathKind::Proving,
                    Some("zermatt") => PathKind::Zermatt,
                    Some("blueridge") => PathKind::BlueRidge,
                    Some("grasslab") => PathKind::GrassLab,
                    v => return Err(format!("--path: expected orbit|static|proving|zermatt|blueridge|grasslab, got {v:?}")),
                }
            }
            other => return Err(format!("unknown argument {other:?} (see --help)")),
        }
    }
    if a.width == 0 || a.height == 0 {
        return Err("--width and --height must be > 0".into());
    }
    if a.play.is_some() && a.headless {
        return Err("--play opens a window; for headless terrain use --path zermatt|blueridge|grasslab".into());
    }
    Ok(Some(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Option<Args>, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_headless_run() {
        let a = p("--headless --empty --width 2560 --height 1440 --frames 300 --out x/y --path static").unwrap().unwrap();
        assert!(a.headless && a.empty);
        assert_eq!((a.width, a.height, a.frames), (2560, 1440, Some(300)));
        assert_eq!(a.out, PathBuf::from("x/y"));
        assert_eq!(a.path, PathKind::Static);
    }

    #[test]
    fn parses_m2_flags_and_picks_the_scene() {
        let a = p("--headless --path proving --lod-fade 0 --no-occlusion --pop --speed 0.25 --lod-px 2").unwrap().unwrap();
        assert_eq!((a.path, a.scene(), a.lod_fade, a.no_occlusion, a.pop, a.speed, a.lod_px), (PathKind::Proving, SceneKind::Proving, Some(0), true, true, 0.25, Some(2.0)));
        assert_eq!(p("--headless").unwrap().unwrap().scene(), SceneKind::Test);
        assert_eq!(p("").unwrap().unwrap().scene(), SceneKind::Proving);
        assert_eq!(p("--scene test").unwrap().unwrap().scene(), SceneKind::Test);
        assert!(p("--lod-fade 64").is_err());
        assert!(p("--scene moon").is_err());
    }

    #[test]
    fn parses_grasslab() {
        assert_eq!(p("--play grasslab").unwrap().unwrap().play, Some(PathKind::GrassLab));
        assert_eq!(p("--path grasslab").unwrap().unwrap().path, PathKind::GrassLab);
    }

    #[test]
    fn parses_character() {
        assert_eq!(p("--play zermatt").unwrap().unwrap().character, Character::Robot);
        assert_eq!(p("--play zermatt --character solarcat").unwrap().unwrap().character, Character::Solarcat);
        assert!(p("--character dog").is_err());
    }

    #[test]
    fn parses_vsync_and_rejects_junk() {
        assert!(!p("--vsync off").unwrap().unwrap().vsync);
        assert!(p("--vsync maybe").is_err());
        assert!(p("--frames").is_err());
        assert!(p("--bogus").is_err());
        assert!(p("--width 0").is_err());
        assert_eq!(p("--help").unwrap(), None);
        assert_eq!(p("--camera 1,2,3,0.5,-0.1").unwrap().unwrap().camera, Some([1.0, 2.0, 3.0, 0.5, -0.1]));
        assert!(p("--camera 1,2,3").is_err());
        let g = p("--headless --path zermatt --ground 46.02,7.74,90,-5,1.7 --sun 120,8").unwrap().unwrap();
        assert_eq!((g.ground, g.sun), (Some([46.02, 7.74, 90.0, -5.0, 1.7]), Some([120.0, 8.0])));
        assert!(p("--sun 1").is_err());
        assert_eq!(p("--date 2026-10-20").unwrap().unwrap().date, Some((2026, 10, 20)));
        assert!(p("--date 2026-13-01").is_err());
        assert!(p("--date 2026-10").is_err());
        assert_eq!(p("--packs x/p").unwrap().unwrap().packs, PathBuf::from("x/p"));
        assert_eq!(p("--pack a.ewpack --pack b.ewpack").unwrap().unwrap().extra_packs, vec![PathBuf::from("a.ewpack"), PathBuf::from("b.ewpack")]);
        assert!(p("--pack").is_err());
    }
}
