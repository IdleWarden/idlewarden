// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bundle::load_all;
use idlewarden_agent::Condition;
use idlewarden_capture::Size;
use idlewarden_plugin_api::{InputCommand, MouseButton, Observation, Point, Signal};

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
        intents: Vec::new(),
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

fn is_true(signal: &str) -> Condition {
    Condition::IsTrue {
        signal: signal.to_owned(),
    }
}

fn is_false(signal: &str) -> Condition {
    Condition::IsFalse {
        signal: signal.to_owned(),
    }
}

fn collect() -> IntentDraft {
    IntentDraft {
        name: "collect_reward".to_owned(),
        when: vec![is_true("ui.reward_ready")],
        click: Point { x: 0.48, y: 0.68 },
        post_condition: vec![is_false("ui.reward_ready")],
    }
}

fn with_intents(intents: Vec<IntentDraft>) -> Draft {
    Draft {
        intents,
        ..full_draft()
    }
}

fn refused(draft: Draft, name: &str) -> AuthoringError {
    let root = root(name);
    let error = write(&draft, &game_screen(), &root).expect_err("the intent is refused");
    assert_eq!(
        std::fs::read_dir(&root).expect("readable").count(),
        0,
        "a refused intent must be caught before anything is written"
    );
    error
}

#[test]
fn a_drawn_intent_fires_on_the_frame_that_shows_its_trigger() {
    let root = root("intent-fires");
    write(&with_intents(vec![collect()]), &game_screen(), &root).expect("written");

    let (_, bundle) = load_all(&root).into_iter().next().expect("found");
    let bundle = bundle.expect("loads");
    let observation = Observation {
        frame_id: 1,
        captured_at_ms: 0,
        signals: bundle
            .perceiver()
            .perceive(&game_screen())
            .expect("perceived")
            .into_iter()
            .map(|extracted| Signal {
                id: extracted.id,
                value: extracted.value,
                confidence: extracted.confidence,
            })
            .collect(),
    };

    let tick = bundle.tree().tick(&observation);

    assert_eq!(
        tick.intent.map(|intent| intent.name),
        Some("collect_reward".to_owned()),
        "the gold patch is lit, so the drawn intent has to be proposed"
    );
}

#[test]
fn a_drawn_intent_clicks_the_picked_point_and_asks_for_the_mouse() {
    let root = root("intent-manifest");
    let dir = write(&with_intents(vec![collect()]), &game_screen(), &root).expect("written");

    let manifest: PluginManifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("plugin.json")).expect("readable"))
            .expect("parses");
    assert_eq!(manifest.intents, vec!["collect_reward".to_owned()]);
    assert!(
        manifest.capabilities.contains(&Capability::InputMouse),
        "a plugin that clicks has to say so, or the host refuses its input"
    );

    let rules = PluginRules::load(&dir.join("rules.json")).expect("valid rules");
    assert_eq!(
        rules.intents[0].commands[0],
        InputCommand::Click {
            at: Point { x: 0.48, y: 0.68 },
            button: MouseButton::Left,
        }
    );
}

#[test]
fn an_intent_with_no_trigger_is_refused() {
    let mut always = collect();
    always.when.clear();

    assert!(matches!(
        refused(with_intents(vec![always]), "no-trigger"),
        AuthoringError::NoTrigger(name) if name == "collect_reward"
    ));
}

#[test]
fn an_intent_with_no_post_condition_is_refused() {
    let mut unconfirmed = collect();
    unconfirmed.post_condition.clear();

    assert!(matches!(
        refused(with_intents(vec![unconfirmed]), "no-post"),
        AuthoringError::NoPostCondition(_)
    ));
}

#[test]
fn a_post_condition_that_was_already_true_before_is_refused() {
    let mut circular = collect();
    circular.post_condition = vec![is_true("ui.reward_ready")];

    assert!(
        matches!(
            refused(with_intents(vec![circular]), "circular"),
            AuthoringError::UnprovablePostCondition(_)
        ),
        "it would confirm a click that changed nothing"
    );
}

#[test]
fn a_post_condition_adding_to_the_trigger_still_counts_as_evidence() {
    let root = root("adds-evidence");
    let mut stronger = collect();
    stronger.post_condition = vec![is_true("ui.reward_ready"), is_true("ui.menu_open")];

    assert!(write(&with_intents(vec![stronger]), &game_screen(), &root).is_ok());
}

#[test]
fn an_intent_on_a_signal_that_was_not_drawn_is_refused() {
    let mut stray = collect();
    stray.when = vec![is_true("ui.never_drawn")];

    assert!(matches!(
        refused(with_intents(vec![stray]), "unknown"),
        AuthoringError::UnknownSignal(_, signal) if signal == "ui.never_drawn"
    ));
}

#[test]
fn an_anchor_is_not_a_signal_an_intent_can_test() {
    let mut on_anchor = collect();
    on_anchor.when = vec![is_true("logo")];

    assert!(matches!(
        refused(with_intents(vec![on_anchor]), "anchor"),
        AuthoringError::UnknownSignal(_, signal) if signal == "logo"
    ));
}

#[test]
fn a_numeric_condition_on_a_drawn_signal_is_refused() {
    let mut numeric = collect();
    numeric.when = vec![Condition::AtLeast {
        signal: "ui.reward_ready".to_owned(),
        value: 3.0,
    }];

    assert!(matches!(
        refused(with_intents(vec![numeric]), "numeric"),
        AuthoringError::UnsupportedCondition(_, _)
    ));
}

#[test]
fn a_post_condition_that_says_a_counter_moved_is_accepted() {
    let mut counted = collect();
    counted.post_condition = vec![Condition::Increased {
        signal: "ui.reward_ready".to_owned(),
    }];

    let written = write(
        &with_intents(vec![counted]),
        &game_screen(),
        &root("counter-moved"),
    )
    .expect("a drawn signal can be checked for having moved");

    let rules = std::fs::read_to_string(written.join("rules.json")).expect("rules written");
    assert!(rules.contains("\"increased\""), "{rules}");
}

#[test]
fn deciding_on_a_counter_having_moved_is_refused() {
    let mut counted = collect();
    counted.when = vec![Condition::Increased {
        signal: "ui.reward_ready".to_owned(),
    }];

    assert!(matches!(
        refused(with_intents(vec![counted]), "delta-when"),
        AuthoringError::DeltaWhenDeciding(_, _)
    ));
}

#[test]
fn a_click_outside_the_window_is_refused() {
    let mut beyond = collect();
    beyond.click = Point { x: 1.2, y: 0.5 };

    assert!(matches!(
        refused(with_intents(vec![beyond]), "click-outside"),
        AuthoringError::ClickOutside(_)
    ));
}

#[test]
fn two_intents_with_the_same_name_are_refused() {
    assert!(matches!(
        refused(with_intents(vec![collect(), collect()]), "twin-intents"),
        AuthoringError::BadIntentName(_)
    ));
}

#[test]
fn the_payload_the_editor_sends_is_the_draft_the_core_expects() {
    let sent = r#"{
      "id": "local.demo",
      "name": "Demo",
      "game": { "executable": "Demo.exe" },
      "regions": [
        { "name": "ui.reward_ready", "kind": "color_probe",
          "area": { "x": 0.47, "y": 0.7, "w": 0.09, "h": 0.075 } }
      ],
      "intents": [
        { "name": "action-1",
          "when": [{ "op": "is_true", "signal": "ui.reward_ready" }],
          "click": { "x": 0.5, "y": 0.7200000000000001 },
          "post_condition": [{ "op": "is_false", "signal": "ui.reward_ready" }] }
      ]
    }"#;

    let draft: Draft = serde_json::from_str(sent).expect("the editor's payload deserialises");

    assert_eq!(draft.intents[0].when, vec![is_true("ui.reward_ready")]);
    assert_eq!(
        draft.intents[0].post_condition,
        vec![is_false("ui.reward_ready")]
    );
    assert!(
        validate(&draft).is_ok(),
        "and passes the same validation `write` runs"
    );
}
