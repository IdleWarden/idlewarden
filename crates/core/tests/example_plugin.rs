// SPDX-License-Identifier: MPL-2.0
//! The reference plugin has to load. A format nothing ships an example of is a
//! format nobody can copy.

use std::path::Path;

use idlewarden_core::PluginRules;

#[test]
fn the_reference_plugin_loads() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/example-game/rules.json");

    let rules = PluginRules::load(&path).expect("the shipped example must parse");

    assert_eq!(rules.anchors.len(), 1);
    assert_eq!(rules.signals.len(), 3);
    assert_eq!(rules.recipes().len(), 2);
    assert_eq!(
        rules.templates(),
        vec!["assets/header_main.png", "assets/logo.png"]
    );

    for recipe in rules.recipes() {
        assert!(
            !recipe.post_condition.is_empty(),
            "`{}` would never be confirmable",
            recipe.intent
        );
    }
}
