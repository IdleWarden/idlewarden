// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bundle::load_all;
use idlewarden_capture::Size;

const W: u32 = 100;
const H: u32 = 80;

struct Canvas(Vec<u8>);

impl Canvas {
    fn new() -> Self {
        let mut bgra = Vec::with_capacity((W * H * 4) as usize);
        for y in 0..H {
            for x in 0..W {
                let shade = (((x * 7 + y * 13) % 64) + 40) as u8;
                bgra.extend_from_slice(&[shade, shade, shade, 255]);
            }
        }
        Canvas(bgra)
    }

    fn paint(&mut self, left: u32, top: u32, w: u32, h: u32, pick: impl Fn(u32, u32) -> [u8; 3]) {
        for y in top..top + h {
            for x in left..left + w {
                let [r, g, b] = pick(x - left, y - top);
                let index = ((y * W + x) * 4) as usize;
                self.0[index] = b;
                self.0[index + 1] = g;
                self.0[index + 2] = r;
            }
        }
    }

    fn frame(self) -> Frame {
        Frame {
            id: 1,
            captured_at_ms: 0,
            size: Size {
                width: W,
                height: H,
            },
            bgra: self.0,
        }
    }
}

fn checker(x: u32, y: u32) -> [u8; 3] {
    if (x / 2 + y / 2) % 2 == 0 {
        [250, 250, 250]
    } else {
        [10, 10, 10]
    }
}

fn stripes(x: u32, y: u32) -> [u8; 3] {
    if (x + y) % 3 == 0 {
        [220, 40, 40]
    } else {
        [30, 30, 90]
    }
}

const GOLD: [u8; 3] = [232, 185, 62];

fn game_screen() -> Frame {
    let mut canvas = Canvas::new();
    canvas.paint(10, 10, 12, 12, checker);
    canvas.paint(70, 5, 10, 10, stripes);
    canvas.paint(40, 50, 16, 10, |_, _| GOLD);
    canvas.frame()
}

fn area(left: u32, top: u32, w: u32, h: u32) -> Roi {
    Roi {
        x: left as f64 / W as f64,
        y: top as f64 / H as f64,
        w: w as f64 / W as f64,
        h: h as f64 / H as f64,
    }
}

fn region(name: &str, kind: RegionKind, roi: Roi) -> Region {
    Region {
        name: name.to_owned(),
        kind,
        area: roi,
    }
}

fn draft(regions: Vec<Region>) -> Draft {
    Draft {
        id: PluginId("local.drawn-game".to_owned()),
        name: "Drawn Game".to_owned(),
        game: GameMatcher {
            executable: Some("DrawnGame.exe".to_owned()),
            ..Default::default()
        },
        regions,
    }
}

fn full_draft() -> Draft {
    draft(vec![
        region("logo", RegionKind::Anchor, area(70, 5, 10, 10)),
        region(
            "ui.menu_open",
            RegionKind::TemplateMatch,
            area(10, 10, 12, 12),
        ),
        region(
            "ui.reward_ready",
            RegionKind::ColorProbe,
            area(40, 50, 16, 10),
        ),
    ])
}

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "idlewarden-authoring-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("the plugin root could be created");
    root
}

#[test]
fn a_drawn_plugin_loads_like_any_other_and_recognises_its_own_frame() {
    let root = root("happy");

    let dir = write(&full_draft(), &game_screen(), &root).expect("a valid draft is written");

    let loaded = load_all(&root);
    assert_eq!(loaded.len(), 1, "the app's own loader has to find it");
    let bundle = loaded
        .into_iter()
        .next()
        .and_then(|(_, bundle)| bundle.ok())
        .expect("and load it without error");

    let extracted = bundle
        .perceiver()
        .perceive(&game_screen())
        .expect("the anchor is found on the frame it was cut from");
    assert_eq!(extracted.len(), 2);
    assert!(extracted
        .iter()
        .all(|signal| signal.value == Value::Bool(true)));

    assert!(dir.join("assets/logo.png").is_file());
    assert!(dir.join("assets/ui.menu_open.png").is_file());
}

#[test]
fn the_manifest_names_the_game_and_declares_every_signal() {
    let root = root("manifest");
    let dir = write(&full_draft(), &game_screen(), &root).expect("written");

    let manifest: PluginManifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("plugin.json")).expect("readable"))
            .expect("the manifest parses back");

    assert_eq!(manifest.game.executable.as_deref(), Some("DrawnGame.exe"));
    let declared: Vec<&str> = manifest.signals.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(declared, vec!["ui.menu_open", "ui.reward_ready"]);
    assert!(
        manifest.intents.is_empty(),
        "the editor draws what to look at, it does not invent actions"
    );
    assert_eq!(manifest.capabilities, vec![Capability::Capture]);
}

#[test]
fn a_featureless_template_is_refused_and_nothing_is_left_behind() {
    let root = root("flat");
    let mut canvas = Canvas::new();
    canvas.paint(10, 10, 12, 12, |_, _| [128, 128, 128]);

    let error = write(
        &draft(vec![region(
            "ui.blank",
            RegionKind::TemplateMatch,
            area(10, 10, 12, 12),
        )]),
        &canvas.frame(),
        &root,
    )
    .expect_err("a flat crop matches nothing, not even itself");

    assert!(matches!(error, AuthoringError::Unrecognised(name, _) if name == "ui.blank"));
    assert!(
        !root.join("local.drawn-game").exists(),
        "a plugin that cannot see its own frame must not be left for the loader"
    );
}

#[test]
fn a_colour_probe_over_mixed_pixels_is_refused() {
    let root = root("mixed");

    let error = write(
        &draft(vec![region(
            "ui.patchy",
            RegionKind::ColorProbe,
            area(10, 10, 12, 12),
        )]),
        &game_screen(),
        &root,
    )
    .expect_err("the average of black and white matches neither");

    assert!(matches!(error, AuthoringError::Unrecognised(name, _) if name == "ui.patchy"));
    assert!(!root.join("local.drawn-game").exists());
}

#[test]
fn an_existing_plugin_is_never_overwritten() {
    let root = root("exists");
    let existing = root.join("local.drawn-game");
    std::fs::create_dir_all(&existing).expect("created");
    std::fs::write(existing.join("plugin.json"), "{\"keep\": true}").expect("written");

    let error = write(&full_draft(), &game_screen(), &root).expect_err("the id is taken");

    assert!(matches!(error, AuthoringError::Exists(_)));
    assert_eq!(
        std::fs::read_to_string(existing.join("plugin.json")).expect("still there"),
        "{\"keep\": true}"
    );
}

#[test]
fn an_id_that_is_not_reverse_dns_is_refused_before_touching_the_disk() {
    let root = root("bad-id");
    let mut bad = full_draft();
    bad.id = PluginId("My Game".to_owned());

    let error = write(&bad, &game_screen(), &root).expect_err("not reverse-DNS");

    assert!(matches!(error, AuthoringError::BadId(_)));
    assert_eq!(std::fs::read_dir(&root).expect("readable").count(), 0);
}

#[test]
fn a_draft_that_matches_no_window_is_refused() {
    let root = root("no-matcher");
    let mut blind = full_draft();
    blind.game = GameMatcher::default();

    assert!(matches!(
        write(&blind, &game_screen(), &root),
        Err(AuthoringError::NoMatcher)
    ));
}

#[test]
fn two_regions_with_the_same_name_are_refused() {
    let root = root("duplicate");
    let twice = draft(vec![
        region("ui.x", RegionKind::ColorProbe, area(40, 50, 16, 10)),
        region("ui.x", RegionKind::ColorProbe, area(40, 50, 16, 10)),
    ]);

    assert!(matches!(
        write(&twice, &game_screen(), &root),
        Err(AuthoringError::BadRegionName(name)) if name == "ui.x"
    ));
}

#[test]
fn a_region_name_that_would_escape_the_assets_folder_is_refused() {
    let root = root("traversal");
    let escaping = draft(vec![region(
        "../../evil",
        RegionKind::TemplateMatch,
        area(10, 10, 12, 12),
    )]);

    assert!(matches!(
        write(&escaping, &game_screen(), &root),
        Err(AuthoringError::BadRegionName(_))
    ));
}

#[test]
fn a_region_outside_the_frame_is_refused() {
    let root = root("outside");
    let beyond = draft(vec![region(
        "ui.beyond",
        RegionKind::ColorProbe,
        Roi {
            x: 0.9,
            y: 0.9,
            w: 0.2,
            h: 0.2,
        },
    )]);

    assert!(matches!(
        write(&beyond, &game_screen(), &root),
        Err(AuthoringError::OutOfFrame(name)) if name == "ui.beyond"
    ));
}

#[test]
fn widening_a_search_area_keeps_its_centre_even_against_the_frame_edge() {
    let at_edge = Roi {
        x: 0.02,
        y: 0.5,
        w: 0.2,
        h: 0.1,
    };

    let widened = regions::widened(at_edge);

    let centre = |roi: Roi| (roi.x + roi.w / 2.0, roi.y + roi.h / 2.0);
    let (before, after) = (centre(at_edge), centre(widened));
    assert!(
        (before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9,
        "an off-centre search area reads as the layout having moved: {before:?} vs {after:?}"
    );
    assert!(widened.is_within_unit_square());
    assert!(widened.h > at_edge.h, "the free axis still gets its margin");
}
