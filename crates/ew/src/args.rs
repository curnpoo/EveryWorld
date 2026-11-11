//! Command line (hand-rolled; no dependency).

use crate::camera::PathKind;
use std::path::PathBuf;

pub const USAGE: &str = "\
ew: EveryWorld engine shell (Phase 1, M1b)

USAGE:
  ew [OPTIONS]                 open a window with the test scene
  ew --headless [OPTIONS]      render offscreen, no window, write PNG + CSV + summary

OPTIONS:
  --width W, --height H        output size in pixels (default 1280x720)
  --frames N                   stop after N frames (windowed: presented frames; 0 = run
                               until closed; headless default 300)
  --vsync on|off               windowed: vsync on (default) or off (uncapped). V toggles it
  --headless                   offscreen harness (P1-D15)
  --out DIR                    headless output directory (default target/headless)
  --path orbit|static          headless camera path (default orbit)
  --empty                      clear + tonemap only (the empty-frame budget)
  --validation                 GPU validation on (default in debug builds)
  --frames-in-flight 2|3       default 2
  -h, --help

KEYS (windowed):
  W A S D move, Q/E down/up, Shift faster, hold left mouse to look, V vsync, Esc quit";

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
            "--width" => a.width = num(&arg, it.next())?,
            "--height" => a.height = num(&arg, it.next())?,
            "--frames" => a.frames = Some(num(&arg, it.next())?),
            "--frames-in-flight" => a.frames_in_flight = num(&arg, it.next())?,
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
                    v => return Err(format!("--path: expected orbit|static, got {v:?}")),
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
    fn parses_vsync_and_rejects_junk() {
        assert!(!p("--vsync off").unwrap().unwrap().vsync);
        assert!(p("--vsync maybe").is_err());
        assert!(p("--frames").is_err());
        assert!(p("--bogus").is_err());
        assert!(p("--width 0").is_err());
        assert_eq!(p("--help").unwrap(), None);
    }
}
