// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

pub(crate) fn vdf_value(line: &str, key: &str) -> Option<String> {
    let mut parts = line.split('"').filter(|part| !part.trim().is_empty());
    let found = parts.next()?;
    if !found.eq_ignore_ascii_case(key) {
        return None;
    }
    Some(parts.next()?.replace("\\\\", "\\"))
}

pub(crate) fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| vdf_value(line, "path"))
        .map(PathBuf::from)
        .collect()
}

pub(crate) fn app_manifest(acf: &str) -> Option<(u32, String)> {
    let mut appid = None;
    let mut installdir = None;

    for line in acf.lines() {
        if let Some(value) = vdf_value(line, "appid") {
            appid = value.parse().ok();
        } else if let Some(value) = vdf_value(line, "installdir") {
            installdir = Some(value);
        }
    }

    Some((appid?, installdir?))
}

pub(crate) fn owns(install_dir: &Path, executable: &Path) -> bool {
    let install = install_dir.to_string_lossy().to_lowercase();
    let exe = executable.to_string_lossy().to_lowercase();
    !install.is_empty() && exe.starts_with(&install)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_file_yields_every_path() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;

        assert_eq!(
            library_paths(vdf),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn a_manifest_yields_the_appid_and_the_install_directory() {
        let acf = r#"
"AppState"
{
	"appid"		"570"
	"name"		"Some Game"
	"installdir"		"Some Game"
}
"#;

        assert_eq!(app_manifest(acf), Some((570, "Some Game".to_owned())));
    }

    #[test]
    fn a_manifest_missing_a_field_is_not_half_parsed() {
        let acf = "\"AppState\"\n{\n\t\"name\"\t\t\"Some Game\"\n}\n";

        assert_eq!(app_manifest(acf), None);
    }

    #[test]
    fn a_key_is_not_matched_inside_another_key() {
        let line = "\t\"steam_appid_override\"\t\t\"1\"";

        assert_eq!(vdf_value(line, "appid"), None);
    }

    #[test]
    fn ownership_is_a_case_insensitive_prefix() {
        let install = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Some Game");

        assert!(owns(
            &install,
            Path::new(r"d:\steamlibrary\steamapps\common\some game\bin\game.exe")
        ));
        assert!(!owns(
            &install,
            Path::new(r"D:\SteamLibrary\steamapps\common\Other Game\game.exe")
        ));
    }

    #[test]
    fn an_empty_install_directory_owns_nothing() {
        assert!(!owns(Path::new(""), Path::new(r"C:\anything\game.exe")));
    }
}
