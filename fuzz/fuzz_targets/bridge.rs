// SPDX-License-Identifier: MPL-2.0
#![no_main]

use std::io::{self, Cursor};

use idlewarden_bridge::transport::LineTransport;
use idlewarden_bridge::Bridge;
use idlewarden_plugin_api::Intent;
use libfuzzer_sys::fuzz_target;

const HELLO: &[u8] =
    b"{\"response\":\"hello\",\"plugin\":\"dev.example.game\",\"api_version\":\"^0.1\"}\n";

fuzz_target!(|replies: &[u8]| {
    let script = [HELLO, replies].concat();
    let transport = LineTransport::new(Cursor::new(script), io::sink());
    let mut bridge = Bridge::open(Box::new(transport)).expect("the canned hello must be accepted");
    for now_ms in 0..4 {
        let _ = bridge.observe(now_ms);
    }
    let _ = bridge.act(&Intent::new("buy_upgrade"));
});
