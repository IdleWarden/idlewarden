// SPDX-License-Identifier: MPL-2.0
#![no_main]

use idlewarden_core::PluginRules;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|json: &str| {
    if let Ok(rules) = PluginRules::parse(json) {
        let _ = rules.tree();
    }
});
