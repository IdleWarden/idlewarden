// SPDX-License-Identifier: MPL-2.0
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use idlewarden_plugin_api::PluginId;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::*;

const MOD_ID: &str = "dev.idlewarden.reference-mod";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("idlewarden-mod-install")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn zipped(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in files {
        writer.start_file(*name, options).expect("entry");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("archive").into_inner()
}

fn reference_mod() -> Vec<u8> {
    zipped(&[
        ("IdleWarden.Reference.dll", b"reference"),
        ("IdleWarden.Bridge.dll", b"bridge"),
    ])
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
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

fn bepinex_game(name: &str) -> PathBuf {
    let game = scratch(name);
    std::fs::create_dir_all(game.join("BepInEx").join("core")).expect("loader");
    game
}

mod index {
    use super::*;

    fn index(versions: serde_json::Value) -> Index {
        serde_json::from_value(serde_json::json!({
            "schema_version": "1.1.0",
            "plugins": [],
            "mods": [{
                "id": MOD_ID,
                "name": "Reference",
                "plugin": "dev.idlewarden.reference",
                "bridge": { "name": "reference" },
                "loader": "bepinex",
                "source": { "repository": "https://github.com/IdleWarden/idlewarden" },
                "versions": versions,
            }],
        }))
        .expect("the index parses")
    }

    fn version(version: &str, api: &str, yanked: bool) -> serde_json::Value {
        serde_json::json!({
            "version": version,
            "api_version": api,
            "url": format!("https://example.com/{version}.zip"),
            "sha256": "0".repeat(64),
            "attestation": { "repository": "IdleWarden/idlewarden" },
            "yanked": yanked,
        })
    }

    fn picked(index: &Index) -> Option<String> {
        index
            .release_for(
                &PluginId("dev.idlewarden.reference".to_owned()),
                "reference",
            )
            .map(|release| release.version.version.to_string())
    }

    #[test]
    fn the_newest_release_is_picked_by_version_not_by_listing_order() {
        let index = index(serde_json::json!([
            version("26.9.2", "^0.1", false),
            version("26.10.1", "^0.1", false),
            version("26.9.10", "^0.1", false),
        ]));

        assert_eq!(picked(&index).as_deref(), Some("26.10.1"));
    }

    #[test]
    fn a_yanked_release_is_never_installed() {
        let index = index(serde_json::json!([
            version("26.9.1", "^0.1", false),
            version("26.9.2", "^0.1", true),
        ]));

        assert_eq!(picked(&index).as_deref(), Some("26.9.1"));
    }

    #[test]
    fn a_release_built_for_another_protocol_is_skipped() {
        let index = index(serde_json::json!([
            version("26.9.1", "^0.1", false),
            version("26.9.2", "^0.2", false),
        ]));

        assert_eq!(
            picked(&index).as_deref(),
            Some("26.9.1"),
            "installing a mod the handshake will refuse only moves the failure later"
        );
    }

    #[test]
    fn a_mod_for_another_endpoint_does_not_serve_this_bridge() {
        let index = index(serde_json::json!([version("26.9.1", "^0.1", false)]));

        assert!(index
            .release_for(&PluginId("dev.idlewarden.reference".to_owned()), "other")
            .is_none());
        assert!(index
            .release_for(&PluginId("dev.someone.else".to_owned()), "reference")
            .is_none());
    }
}

mod loader {
    use super::*;

    #[test]
    fn a_bepinex_mod_gets_its_own_folder_under_plugins() {
        let game = bepinex_game("bepinex");

        let destination = Loader::Bepinex
            .destination(&game, None, MOD_ID)
            .expect("BepInEx is installed");

        assert_eq!(
            destination,
            Destination::Own(game.join("BepInEx").join("plugins").join(MOD_ID))
        );
    }

    #[test]
    fn a_game_without_the_loader_is_refused_and_says_where_it_looked() {
        let game = scratch("no-loader");

        let error = Loader::Bepinex
            .destination(&game, None, MOD_ID)
            .expect_err("no BepInEx here");

        assert!(
            error.to_string().contains("BepInEx")
                && error.to_string().contains(&game.display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn melonloader_mods_share_the_mods_folder() {
        let game = scratch("melon");
        std::fs::create_dir_all(game.join("MelonLoader")).expect("loader");

        assert_eq!(
            Loader::Melonloader.destination(&game, None, MOD_ID),
            Ok(Destination::Shared(game.join("Mods")))
        );
    }

    #[test]
    fn reloaded_needs_its_own_mods_folder_to_exist() {
        let game = scratch("reloaded-game");
        let reloaded = scratch("reloaded-mods");

        assert_eq!(
            Loader::ReloadedIi.destination(&game, None, MOD_ID),
            Err(LoaderError::ReloadedMissing)
        );
        assert_eq!(
            Loader::ReloadedIi.destination(&game, Some(&reloaded), MOD_ID),
            Ok(Destination::Own(reloaded.join(MOD_ID)))
        );
    }

    #[test]
    fn a_manual_mod_is_never_installed_by_the_app() {
        assert_eq!(
            Loader::Manual.destination(&scratch("manual"), None, MOD_ID),
            Err(LoaderError::Manual)
        );
    }

    #[test]
    fn the_registry_spelling_of_each_loader_parses() {
        let loaders: Vec<Loader> =
            serde_json::from_str(r#"["bepinex", "melonloader", "reloaded-ii", "manual"]"#)
                .expect("parses");

        assert_eq!(
            loaders,
            [
                Loader::Bepinex,
                Loader::Melonloader,
                Loader::ReloadedIi,
                Loader::Manual
            ]
        );
    }
}

mod archive {
    use super::*;

    fn own(game: &Path) -> Destination {
        Loader::Bepinex
            .destination(game, None, MOD_ID)
            .expect("BepInEx is installed")
    }

    #[test]
    fn a_verified_archive_lands_in_the_loader_folder() {
        let game = bepinex_game("verified");
        let archive = reference_mod();
        let destination = own(&game);

        let installed =
            install(&archive, &sha256_hex(&archive), MOD_ID, &destination).expect("installs");

        assert_eq!(installed, destination.path());
        assert_eq!(
            listing(&installed),
            ["IdleWarden.Bridge.dll", "IdleWarden.Reference.dll"]
        );
        assert_eq!(
            listing(&game.join("BepInEx").join("plugins")),
            [MOD_ID],
            "no staging folder may be left behind"
        );
    }

    #[test]
    fn a_checksum_mismatch_writes_nothing_at_all() {
        let game = bepinex_game("tampered");
        let archive = reference_mod();

        let error = install(&archive, &"0".repeat(64), MOD_ID, &own(&game))
            .expect_err("the digest does not match");

        assert!(matches!(error, InstallError::Checksum { .. }));
        assert!(
            !game.join("BepInEx").join("plugins").exists(),
            "the checksum is checked before anything touches the disk"
        );
    }

    #[test]
    fn the_checksum_is_compared_without_case() {
        let game = bepinex_game("upper");
        let archive = reference_mod();

        install(
            &archive,
            &sha256_hex(&archive).to_uppercase(),
            MOD_ID,
            &own(&game),
        )
        .expect("a registry entry in capitals is still the same digest");
    }

    #[test]
    fn reinstalling_replaces_the_previous_version_entirely() {
        let game = bepinex_game("upgrade");
        let destination = own(&game);
        let old = zipped(&[
            ("IdleWarden.Reference.dll", b"old"),
            ("Removed.dll", b"gone"),
        ]);
        install(&old, &sha256_hex(&old), MOD_ID, &destination).expect("first install");

        let new = reference_mod();
        install(&new, &sha256_hex(&new), MOD_ID, &destination).expect("upgrade");

        assert_eq!(
            listing(destination.path()),
            ["IdleWarden.Bridge.dll", "IdleWarden.Reference.dll"],
            "a stale DLL from the old version would load next to the new one"
        );
    }

    #[test]
    fn a_shared_folder_keeps_the_other_mods_in_it() {
        let game = scratch("shared");
        let mods = game.join("Mods");
        std::fs::create_dir_all(&mods).expect("mods");
        std::fs::write(mods.join("SomeoneElse.dll"), b"theirs").expect("other mod");
        let archive = reference_mod();

        install(
            &archive,
            &sha256_hex(&archive),
            MOD_ID,
            &Destination::Shared(mods.clone()),
        )
        .expect("installs");

        assert_eq!(
            listing(&mods),
            [
                "IdleWarden.Bridge.dll",
                "IdleWarden.Reference.dll",
                "SomeoneElse.dll"
            ]
        );
    }

    #[test]
    fn an_entry_that_climbs_out_of_the_folder_is_refused_before_writing() {
        let game = bepinex_game("slip");
        let archive = zipped(&[
            ("IdleWarden.Reference.dll", b"ok"),
            ("../../evil.dll", b"x"),
        ]);

        let error =
            install(&archive, &sha256_hex(&archive), MOD_ID, &own(&game)).expect_err("zip slip");

        assert!(matches!(error, InstallError::Escapes(name) if name == "../../evil.dll"));
        assert!(!game.join("BepInEx").join("plugins").exists());
        assert!(!game.join("BepInEx").join("evil.dll").exists());
    }

    #[test]
    fn an_absolute_entry_is_refused() {
        let game = bepinex_game("absolute");
        let archive = zipped(&[("/etc/evil.dll", b"x")]);

        let error = install(&archive, &sha256_hex(&archive), MOD_ID, &own(&game))
            .expect_err("absolute path");

        assert!(matches!(error, InstallError::Escapes(_)));
    }

    #[test]
    fn an_archive_that_unpacks_past_the_ceiling_is_refused() {
        let game = bepinex_game("bomb");
        let zeros = vec![0u8; LARGEST_EXTRACTED as usize + 1];
        let archive = zipped(&[("Bomb.dll", &zeros)]);

        let error =
            install(&archive, &sha256_hex(&archive), MOD_ID, &own(&game)).expect_err("too large");

        assert!(matches!(error, InstallError::TooLarge));
        assert!(!game.join("BepInEx").join("plugins").exists());
    }

    #[test]
    fn an_id_that_is_not_a_plain_folder_name_is_refused() {
        let game = bepinex_game("bad-id");
        let archive = reference_mod();

        for id in ["../escape", "", ".hidden", "Upper.Case", "a/b"] {
            let error = install(&archive, &sha256_hex(&archive), id, &own(&game))
                .expect_err("a registry id is reverse-DNS, lowercase");
            assert!(matches!(error, InstallError::InvalidId(_)), "{id}: {error}");
        }
    }

    #[test]
    fn something_that_is_not_a_zip_is_refused() {
        let game = bepinex_game("not-zip");
        let bytes = b"MZ this is a bare dll".to_vec();

        let error =
            install(&bytes, &sha256_hex(&bytes), MOD_ID, &own(&game)).expect_err("not an archive");

        assert!(matches!(error, InstallError::Unreadable(_)));
    }
}
