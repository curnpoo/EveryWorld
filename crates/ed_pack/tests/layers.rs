//! Override layer files (`docs/contracts/layers.md`, test L1): parse, canonical round trip,
//! malformed operations rejected with line numbers, directory layout.

use std::path::Path;

use ed_geo::{BodyId, NodeId, ObjectId};
use ed_pack::layers::*;

/// Every ```toml block in the contract, in order.
fn contract_examples() -> Vec<String> {
    let doc = include_str!("../../../docs/contracts/layers.md");
    let mut out = Vec::new();
    let mut rest = doc;
    while let Some(i) = rest.find("```toml\n") {
        let body = &rest[i + 8..];
        let end = body.find("```").expect("unterminated block");
        out.push(body[..end].to_string());
        rest = &body[end + 3..];
    }
    out
}

#[test]
fn contract_example_verbatim_has_elided_ids() {
    // The first example in section 3 writes ids as `8f3a...c1` (elided): not valid ObjectIds.
    let ex = &contract_examples()[0];
    assert!(ex.contains("8f3a...c1"));
    let e = LayerFile::parse(ex).unwrap_err();
    assert_eq!(e.line, 2, "{e}");
    assert!(matches!(e.kind, LayerErrorKind::InvalidId(ref s) if s == "8f3a...c1"), "{e}");
}

#[test]
fn contract_examples_with_real_ids_round_trip_byte_identically() {
    let ex = contract_examples();
    assert!(ex.len() >= 2, "the canonical example is appended to layers.md");
    for (i, text) in ex.iter().enumerate().skip(1) {
        let f = LayerFile::parse(text).unwrap_or_else(|e| panic!("example {i}: {e}"));
        assert_eq!(&f.to_toml(), text, "example {i} is not canonical");
    }
    // The canonical twin of the verbatim example has the same content.
    let f = LayerFile::parse(&ex[1]).unwrap();
    assert_eq!(f.ops.len(), 2);
    let OpBody::Place(p) = &f.ops[0].body else { panic!("{:?}", f.ops[0]) };
    assert_eq!(p.prefab, "hubs/aix_market_hall");
    assert_eq!(p.position, [4581204.118, 381377.902, 4407066.540]);
    assert_eq!(p.rotation, Some([0.0, 0.0, 0.3827, 0.9239]));
    let OpBody::Marker(m) = &f.ops[1].body else { panic!() };
    assert_eq!((m.name.as_str(), m.kind.as_str()), ("aix_hall_spawn", "spawn"));
    assert!(f.ops.iter().all(|o| o.id.is_authored()));
}

/// Every operation, in canonical form.
const FULL: &str = r##"format = 1
layer = "base_hubs"

[[op]]
id = "80000000000000000000000000000001"
op = "place"
prefab = "hubs/aix_market_hall"
position = [4581204.118, 381377.902, 4407066.54]
rotation = [0.0, 0.0, 0.3827, 0.9239]
scale = [1.0, 1.5, 1.0]
clear = 4.0

[[op]]
id = "80000000000000000000000000000002"
op = "replace"
target = "00000000000000000000000000abcdef"
params = { colour = "#c8b89a", height = 21.5, roof = "mansard", style = "haussmann" }

[[op]]
id = "80000000000000000000000000000003"
op = "replace"
target = "00000000000000000000000000abcdee"
prefab = "hubs/kiosk"

[[op]]
id = "80000000000000000000000000000004"
op = "suppress"
targets = ["00000000000000000000000000000011", "00000000000000000000000000000012"]
volume = { shape = "box", centre = [4581200.0, 381370.0, 4407060.0], half_extents = [10.0, 5.0, 10.0], rotation = [0.0, 0.0, 0.0, 1.0] }

[[op]]
id = "80000000000000000000000000000005"
op = "suppress"
volume = { shape = "prism", points = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.5]], height = 30.0 }

[[op]]
id = "80000000000000000000000000000006"
op = "marker"
name = "aix_hall_pad"
kind = "landing_pad"
position = [4581210.4, 381380.1, 4407070.2]
rotation = [0.0, 0.0, 0.0, 1.0]
volume = { shape = "sphere", centre = [4581210.4, 381380.1, 4407070.2], radius = 12.0 }
tags = ["hub", "vtol"]

[[op]]
id = "80000000000000000000000000000007"
op = "mission"
giver = "80000000000000000000000000000006"
name = "first_delivery"
requires = []
rewards = { credits = 100 }
steps = [{ goto = "80000000000000000000000000000006", text = "Land on the pad" }]

[[op]]
id = "80000000000000000000000000000008"
op = "road_patch"
add = { class = "residential", points = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], tags = { lit = "yes" } }
remove = ["00000000000000000000000000000099"]

[[op]]
id = "80000000000000000000000000000009"
op = "terrain_patch"
falloff = 15.0
height = 212.5
mode = "set"
shape = { circle = { centre = [1.0, 2.0, 3.0], radius = 40.0 } }
surface = "paved"
"##;

#[test]
fn every_operation_round_trips_byte_identically() {
    let f = LayerFile::parse(FULL).unwrap();
    assert_eq!(f.ops.len(), 9);
    assert_eq!(f.to_toml(), FULL);
    assert_eq!(f.header["layer"].as_str(), Some("base_hubs"));
    let names: Vec<_> = f.ops.iter().map(|o| o.name()).collect();
    assert_eq!(
        names,
        ["place", "replace", "replace", "suppress", "suppress", "marker", "mission", "road_patch", "terrain_patch"]
    );
    let OpBody::Replace(r) = &f.ops[1].body else { panic!() };
    assert_eq!(r.target, "00000000000000000000000000abcdef".parse::<ObjectId>().unwrap());
    let ReplaceWith::Params(p) = &r.with else { panic!() };
    assert_eq!(p.height, Some(21.5));
    let OpBody::Suppress(s) = &f.ops[3].body else { panic!() };
    assert_eq!(s.targets.as_ref().unwrap().len(), 2);
    assert!(matches!(s.volume, Some(Volume::Box { .. })));
    let OpBody::Place(p) = &f.ops[0].body else { panic!() };
    assert_eq!(p.scale, Some(Scale::Axes([1.0, 1.5, 1.0])));
    assert_eq!(f.ops[0].anchor(), Some([4581204.118, 381377.902, 4407066.54]));
}

#[test]
fn file_order_does_not_matter() {
    // Operations shuffled and written non-canonically still serialise to the same text.
    let messy = r#"
[[op]]
op = "marker"
position = [1, 2, 3]
kind = "poi"
name = "b"
id = "80000000000000000000000000000002"

[[op]]
id = "80000000000000000000000000000001"
op = "place"
position = [1.0, 2.0, 3.0]
prefab = "x"
scale = 2
"#;
    let f = LayerFile::parse(messy).unwrap();
    let canon = f.to_toml();
    assert_eq!(
        canon,
        "[[op]]\nid = \"80000000000000000000000000000001\"\nop = \"place\"\nprefab = \"x\"\nposition = [1.0, 2.0, 3.0]\nscale = 2.0\n\n[[op]]\nid = \"80000000000000000000000000000002\"\nop = \"marker\"\nname = \"b\"\nkind = \"poi\"\nposition = [1.0, 2.0, 3.0]\n"
    );
    assert_eq!(LayerFile::parse(&canon).unwrap(), f);
    assert_eq!(LayerFile::parse(&canon).unwrap().to_toml(), canon);
    assert_eq!(LayerFile::parse("").unwrap().to_toml(), "");
}

fn err(src: &str) -> LayerError {
    LayerFile::parse(src).unwrap_err()
}

const ID1: &str = "80000000000000000000000000000001";

#[test]
fn malformed_operations_report_their_line() {
    // Unknown op: reported on the `op` line.
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"teleport\"\n"));
    assert_eq!((e.line, &e.kind), (3, &LayerErrorKind::UnknownOp("teleport".into())), "{e}");

    // Missing field: reported on the `[[op]]` line of the second operation.
    let src = format!(
        "[[op]]\nid = \"{ID1}\"\nop = \"marker\"\nname = \"a\"\nkind = \"poi\"\nposition = [0, 0, 0]\n\n[[op]]\nid = \"80000000000000000000000000000002\"\nop = \"place\"\nposition = [0, 0, 0]\n"
    );
    let e = err(&src);
    assert!(matches!(e.kind, LayerErrorKind::MissingField { field: "prefab", .. }), "{e}");
    assert!((8..=11).contains(&e.line), "missing field reported at line {}", e.line);

    // Unknown field: its own line.
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"place\"\nprefab = \"p\"\nposition = [0, 0, 0]\ncolour = \"red\"\n"));
    assert_eq!(e.line, 6, "{e}");
    assert!(matches!(e.kind, LayerErrorKind::UnknownField { ref field, .. } if field == "colour"));

    // Bad id, non-authored id.
    let e = err("[[op]]\nop = \"place\"\nid = \"xyz\"\n");
    assert_eq!(e.line, 3);
    assert!(matches!(e.kind, LayerErrorKind::InvalidId(_)));
    let e = err("\n\n[[op]]\nid = \"00000000000000000000000000000001\"\nop = \"suppress\"\ntargets = [\"00000000000000000000000000000002\"]\n");
    assert_eq!(e.line, 4);
    assert!(matches!(e.kind, LayerErrorKind::NotAuthored(_)));

    // Duplicate id.
    let one = format!("[[op]]\nid = \"{ID1}\"\nop = \"suppress\"\ntargets = [\"00000000000000000000000000000002\"]\n");
    let e = err(&format!("{one}\n{one}"));
    assert!(matches!(e.kind, LayerErrorKind::DuplicateId(_)), "{e}");
    assert!((6..=9).contains(&e.line), "duplicate reported at line {}", e.line);

    // Not a unit quaternion; wrong type; bad volume.
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"place\"\nprefab = \"p\"\nposition = [0, 0, 0]\nrotation = [0, 0, 1, 1]\n"));
    assert_eq!(e.line, 6);
    assert!(matches!(e.kind, LayerErrorKind::Invalid(_)), "{e}");
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"place\"\nprefab = \"p\"\nposition = [0, 0]\n"));
    assert_eq!(e.line, 5);
    assert!(matches!(e.kind, LayerErrorKind::WrongType { .. }), "{e}");
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"suppress\"\nvolume = {{ shape = \"cone\" }}\n"));
    assert_eq!(e.line, 4);
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"suppress\"\n"));
    assert!(matches!(e.kind, LayerErrorKind::MissingField { .. }));
    let e = err(&format!(
        "[[op]]\nid = \"{ID1}\"\nop = \"replace\"\ntarget = \"00000000000000000000000000000002\"\nprefab = \"a\"\nparams = {{ style = \"b\" }}\n"
    ));
    assert!(matches!(e.kind, LayerErrorKind::Invalid(_)), "{e}");

    // TOML syntax error.
    let e = err(&format!("[[op]]\nid = \"{ID1}\"\nop = \"place\n"));
    assert!(matches!(e.kind, LayerErrorKind::Syntax(_)));
    assert_eq!(e.line, 3, "{e}");
}

#[test]
fn layer_index_round_trips_and_validates() {
    let src = "[[layer]]\nname = \"base_hubs\"\nenabled = true\n\n[[layer]]\nname = \"spaceports\"\nenabled = false\n";
    let idx = LayerIndex::parse(src).unwrap();
    assert_eq!(idx.layers.len(), 2);
    assert_eq!(idx.to_toml(), src);
    let e = LayerIndex::parse("[[layer]]\nname = \"Bad Name\"\nenabled = true\n").unwrap_err();
    assert_eq!(e.line, 2);
    assert!(LayerIndex::parse("[[layer]]\nname = \"a\"\nenabled = true\n\n[[layer]]\nname = \"a\"\nenabled = true\n").is_err());
    assert!(LayerIndex::parse("[[layer]]\nname = \"a\"\n").is_err());
}

#[test]
fn directory_layout_round_trips() {
    let earth = BodyId(3);
    let body_of = |n: &str| (n == "earth").then_some(earth);
    let n = NodeId::new(earth, 2, 10, 460, 244).unwrap();
    assert_eq!(node_file_path("earth", n), Path::new("earth/2/10/460/244.toml"));
    assert_eq!(node_from_file_path(Path::new("earth/2/10/460/244.toml"), &body_of), Some(n));
    assert_eq!(node_from_file_path(Path::new("earth/2/11/460/244.toml"), &body_of), None);
    assert_eq!(node_from_file_path(Path::new("earth/2/10/0460/244.toml"), &body_of), None);
    assert_eq!(node_from_file_path(Path::new("mars/2/10/460/244.toml"), &body_of), None);

    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ed_pack_layer_dir");
    let _ = std::fs::remove_dir_all(&dir);
    let f = LayerFile::parse(FULL).unwrap();
    let n2 = NodeId::new(earth, 2, 10, 461, 244).unwrap();
    write_layer_file(&dir, "earth", n2, &f).unwrap();
    let p = write_layer_file(&dir, "earth", n, &f).unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(), FULL);
    assert!(write_layer_file(&dir, "earth", NodeId::new(earth, 2, 11, 1, 1).unwrap(), &f).is_err());
    let all = read_layer_dir(&dir, &body_of).unwrap();
    assert_eq!(all.keys().copied().collect::<Vec<_>>(), [n, n2]);
    assert_eq!(all[&n], f);
    std::fs::write(dir.join("earth/2/10/stray.toml"), "").unwrap();
    assert!(matches!(read_layer_dir(&dir, &body_of), Err(LayerDirError::BadPath(_))));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn layer_file_becomes_a_layer_chunk() {
    use ed_pack::{Chunk, ChunkMeta, kinds::LayerText};
    let f = LayerFile::parse(FULL).unwrap();
    let c = f.to_chunk(&ChunkMeta::new(1, 1)).unwrap();
    let t: LayerText = Chunk::parse(c.bytes().to_vec()).unwrap().decode::<LayerText>().map(|t| LayerText {
        text: std::borrow::Cow::Owned(t.text.into_owned()),
    }).unwrap();
    assert_eq!(t.text, FULL);
    assert_eq!(t.parse().unwrap(), f);
}
