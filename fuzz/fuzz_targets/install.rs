// SPDX-License-Identifier: MPL-2.0
#![no_main]

use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use idlewarden_mod_install::{install, sha256_hex, Destination};
use libfuzzer_sys::fuzz_target;

fn scratch() -> PathBuf {
    std::env::temp_dir().join(format!("idlewarden-fuzz-install-{}", process::id()))
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fuzz_target!(|archive: &[u8]| {
    let root = scratch();
    let game = root.join("game");

    let sha = sha256_hex(archive);
    let shared = Destination::Shared(game.join("mods"));
    let own = Destination::Own(game.join("own"));

    for destination in [shared, own] {
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&game).expect("scratch dir");
        let _ = install(archive, &sha, "fuzzed-mod", &destination);
        assert_eq!(names(&root), ["game"]);
        for name in names(&game) {
            assert!(
                name == "mods" || name == "own",
                "install left `{name}` next to the destination"
            );
        }
    }

    let _ = fs::remove_dir_all(&root);
});
