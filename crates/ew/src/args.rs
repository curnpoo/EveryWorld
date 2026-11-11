//! Command line (hand-rolled; no dependency).

use crate::camera::PathKind;
use std::path::PathBuf;

pub const USAGE: &str = "\
ew: EveryWorld engine shell (Phase 1, M3)

USAGE:
  ew [OPTIONS]                 open a window in the proving ground (fly camera)
  ew --headless [OPTIONS]      render offscreen, no window, write PNG + CSV + summary

OPTIONS:
  --width W, --height H        output size in pixels (default 1280x720)
  --frames N                   stop after N frames (windowed: presented frames; 0 = run
                               until closed; headless default 300)
  --vsync on|off               windowed: vsync on (default) or off (uncapped). V toggles it
  --headless                   offscreen harness (P1-D15)
  --out DIR                    headless output directory (default target/headless)
  --path orbit|static|proving  headless camera path (default orbit; proving implies
                               --scene proving)
  --scene test|proving         what to load (windowed default proving; headless default
                               test, or proving with --path proving)
  --speed X                    camera path speed multiplier (default 1)
  --lod-px PX                  LOD threshold, projected error in pixels (default 1)
  --lod-fade N                 LOD cross-fade length in frames, 0 = hard switch (default 16)
  --no-occlusion               frustum culling only (no depth-pyramid occlusion test)
  --pop                        headless: run the pop detector (per-tile luminance change)
  --empty                      clear + tonemap only (the empty-frame budget)
  --packs DIR                  pack folder (default target/packs): proving.ewpack (required),
                               bench.ewpack and character.ewpack (optional)
  --camera X,Y,Z,YAW,PITCH     headless: fixed camera (metres from the proving origin, y up;
                               radians) instead of the path; the path still drives time
  --validation                 GPU validation on (default in debug builds)
  --frames-in-flight 2|3       default 2
  -h, --help

KEYS (windowed):
  W A S D move, Q/E down/up, Shift faster, hold left mouse to look, right click picks,
  V vsync, Esc quit";

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
    /// Pack folder ([`crate::content`]).
    pub packs: PathBuf,
    /// Fixed headless camera: x, y, z (from the proving origin), yaw, pitch.
    pub camera: Option<[f64; 5]>,
}

impl Args {
    /// The scene to load: explicit `--scene`, else proving for windowed runs and for
    /// `--path proving`, else the test scene.
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
            packs: crate::content::default_pack_dir(),
            camera: None,
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
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--headless" => a.headless = true,
            "--empty" => a.empty = true,
            "--validation" => a.validation = true,
            "--no-occlusion" => a.no_occlusion = true,
            "--pop" => a.pop = true,
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
            "--camera" => {
                let v = it.next().ok_or("--camera needs X,Y,Z,YAW,PITCH")?;
                let n: Vec<f64> = v.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("--camera: bad value {v:?}"))?;
                if n.len() != 5 || n.iter().any(|x| !x.is_finite()) {
                    return Err(format!("--camera: expected 5 numbers X,Y,Z,YAW,PITCH, got {v:?}"));
                }
                a.camera = Some([n[0], n[1], n[2], n[3], n[4]]);
            }
            "--out" => a.out = PathBuf::from(it.next().ok_or("--out needs a value")?),
            "--vsync" => {
                a.vsync = match it.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    v => return Err(format!("--vsync: expected on|off, got {v:?}")),
                }
            }
            "--path" => {
                a.path = match it.next().as_deref() {
                    Some("orbit") => PathKind::Orbit,
                    Some("static") => PathKind::Static,
                    Some("proving") => PathKind::Proving,
                    v => return Err(format!("--path: expected orbit|static|proving, got {v:?}")),
                }
            }
            other => return Err(format!("unknown argument {other:?} (see --help)")),
        }
    }
    if a.width == 0 || a.height == 0 {
        return Err("--width and --height must be > 0".into());
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
    fn parses_vsync_and_rejects_junk() {
        assert!(!p("--vsync off").unwrap().unwrap().vsync);
        assert!(p("--vsync maybe").is_err());
        assert!(p("--frames").is_err());
        assert!(p("--bogus").is_err());
        assert!(p("--width 0").is_err());
        assert_eq!(p("--help").unwrap(), None);
        assert_eq!(p("--camera 1,2,3,0.5,-0.1").unwrap().unwrap().camera, Some([1.0, 2.0, 3.0, 0.5, -0.1]));
        assert!(p("--camera 1,2,3").is_err());
        assert_eq!(p("--packs x/p").unwrap().unwrap().packs, PathBuf::from("x/p"));
    }
}
