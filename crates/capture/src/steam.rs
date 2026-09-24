// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long an index of installed games is trusted before being rebuilt.
/// Enumeration runs several times a second; rescanning every Steam manifest
/// that often is pure disk churn for something that changes when a game is
/// installed.
const LIBRARY_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, Default)]
pub(crate) struct Library {
    games: Vec<(PathBuf, u32)>,
}

impl Library {
    pub(crate) fn cached() -> Self {
        static CACHE: OnceLock<Mutex<Option<(Instant, Library)>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(None));
        let mut held = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some((built, library)) = held.as_ref() {
            if built.elapsed() < LIBRARY_TTL {
                return library.clone();
            }
        }

        let fresh = Library::scan(&roots());
        *held = Some((Instant::now(), fresh.clone()));
        fresh
    }

    /// Every installed game under the given Steam roots, as the directory it
    /// lives in and the appid that claims it.
    pub(crate) fn scan(roots: &[PathBuf]) -> Self {
        let mut libraries: Vec<PathBuf> = Vec::new();

        for root in roots {
            libraries.push(root.clone());
            if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
                libraries.extend(library_paths(&vdf));
            }
        }

        let mut games = Vec::new();
        for library in libraries {
            let apps = library.join("steamapps");
            let Ok(entries) = std::fs::read_dir(&apps) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| extension == "acf") {
                    if let Some((appid, installdir)) = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|acf| app_manifest(&acf))
                    {
                        games.push((apps.join("common").join(installdir), appid));
                    }
                }
            }
        }

        Library { games }
    }

    pub(crate) fn appid_of(&self, executable: &Path) -> Option<u32> {
        self.games
            .iter()
            .find(|(install, _)| owns(install, executable))
            .map(|(_, appid)| *appid)
    }
}

/// Where Steam keeps its own library. Windows records it, and on Linux it is
/// one of a handful of paths under the home directory.
#[cfg(windows)]
fn roots() -> Vec<PathBuf> {
    use windows::Win32::Foundation::MAX_PATH;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    let mut buffer = vec![0u16; MAX_PATH as usize];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Valve\\Steam"),
            windows::core::w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return Vec::new();
    }

    let chars = (size as usize / 2).saturating_sub(1);
    vec![PathBuf::from(String::from_utf16_lossy(&buffer[..chars]))]
}

#[cfg(not(windows))]
fn roots() -> Vec<PathBuf> {
    candidates(
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
    )
}

#[cfg(not(windows))]
fn candidates(home: Option<PathBuf>, data: Option<PathBuf>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(data) = data {
        roots.push(data.join("Steam"));
    }
    if let Some(home) = home {
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
    }
    roots.into_iter().filter(|root| root.is_dir()).collect()
}

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

    fn steam_tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("idlewarden-steam")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("steamapps/common")).expect("a steam tree");
        root
    }

    fn install(root: &Path, appid: u32, dir: &str) {
        std::fs::write(
            root.join(format!("steamapps/appmanifest_{appid}.acf")),
            format!(
                "\"AppState\"
{{
	\"appid\"		\"{appid}\"
	\"installdir\"		\"{dir}\"
}}
"
            ),
        )
        .expect("a manifest");
        std::fs::create_dir_all(root.join("steamapps/common").join(dir)).expect("an install");
    }

    #[test]
    fn a_game_is_matched_to_the_appid_of_the_library_it_sits_in() {
        let root = steam_tree("first");
        let second = steam_tree("second");
        install(&root, 1454400, "Cookie Clicker");
        install(&second, 3419430, "BongoCat");
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"
{{
	\"0\"
	{{
		\"path\"		\"{}\"
	}}
}}
",
                second.display().to_string().replace('\\', "\\\\")
            ),
        )
        .expect("a library file");

        let library = Library::scan(std::slice::from_ref(&root));

        assert_eq!(
            library.appid_of(
                &root
                    .join("steamapps")
                    .join("common")
                    .join("Cookie Clicker")
                    .join("Cookie Clicker.exe")
            ),
            Some(1454400)
        );
        assert_eq!(
            library.appid_of(
                &second
                    .join("steamapps")
                    .join("common")
                    .join("BongoCat")
                    .join("BongoCat.exe")
            ),
            Some(3419430),
            "a second library named by the vdf is part of the index"
        );
        assert_eq!(
            library.appid_of(Path::new("/usr/bin/firefox")),
            None,
            "a binary outside every install belongs to no game"
        );
    }

    #[test]
    fn a_steam_root_that_is_not_there_indexes_nothing_rather_than_failing() {
        let library = Library::scan(&[PathBuf::from("/nowhere/at/all")]);

        assert_eq!(library.appid_of(Path::new("/nowhere/at/all/game")), None);
    }

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
