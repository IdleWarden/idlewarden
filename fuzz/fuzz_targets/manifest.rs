// SPDX-License-Identifier: MPL-2.0
#![no_main]

use idlewarden_plugin_api::manifest::PluginManifest;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|json: &str| {
    let Ok(manifest) = serde_json::from_str::<PluginManifest>(json) else {
        return;
    };
    let _ = manifest.validate();
    let _ = manifest.api_version.is_satisfied_by_host();
    let printed = serde_json::to_string(&manifest).expect("a parsed manifest must serialize");
    let reparsed: PluginManifest =
        serde_json::from_str(&printed).expect("a serialized manifest must parse back");
    assert_eq!(reparsed, manifest);
});
