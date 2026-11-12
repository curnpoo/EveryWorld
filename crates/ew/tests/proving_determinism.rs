//! Two runs of the proving path on two devices produce byte-identical frames (layout, LOD
//! selection, cross-fade state, culling and the visibility resolve are all deterministic).

use ew::args::Args;
use ew::camera::PathKind;

mod common;

fn run(out: &str) -> ew::headless::Report {
    let args = Args {
        headless: true,
        width: 480,
        height: 270,
        frames: Some(130),
        path: PathKind::Proving,
        out: std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(out),
        packs: common::packs(),
        ..Args::default()
    };
    ew::headless::run(&args).unwrap_or_else(|e| panic!("{out}: {e}"))
}

#[test]
fn proving_frames_are_byte_identical_across_runs() {
    if !common::gpu() {
        return;
    }
    let a = run("det_a");
    let b = run("det_b");
    let street = |r: &ew::headless::Report| r.shots.iter().find(|s| s.0 == "street").expect("street shot at frame 120").1.clone();
    for (pa, pb) in [(a.frame_png.clone(), b.frame_png.clone()), (street(&a), street(&b))] {
        let (x, y) = (std::fs::read(&pa).unwrap(), std::fs::read(&pb).unwrap());
        assert!(x.len() > 1000);
        assert!(x == y, "{} and {} differ", pa.display(), pb.display());
    }
    // The frames are not trivially empty: the street shot has many distinct colours.
    let d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(street(&a)).unwrap()));
    let mut rd = d.read_info().unwrap();
    let mut buf = vec![0; rd.output_buffer_size().unwrap()];
    rd.next_frame(&mut buf).unwrap();
    let mut colours: Vec<[u8; 3]> = buf.as_chunks::<4>().0.iter().map(|p| [p[0], p[1], p[2]]).collect();
    colours.sort_unstable();
    colours.dedup();
    assert!(colours.len() > 500, "{} colours", colours.len());
}
