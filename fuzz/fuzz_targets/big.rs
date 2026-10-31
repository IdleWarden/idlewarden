// SPDX-License-Identifier: MPL-2.0
#![no_main]

use std::cmp::Ordering;

use idlewarden_plugin_api::Big;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let Ok(big) = text.parse::<Big>() else {
        return;
    };
    assert_eq!(big.compare(big), Ordering::Equal);
    let printed = big.to_string();
    let reparsed: Big = printed.parse().expect("a printed Big must parse back");
    assert_eq!(reparsed, big);
    let _ = big.to_f64();
});
