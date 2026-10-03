// SPDX-License-Identifier: MPL-2.0
use std::ffi::OsString;
use std::path::PathBuf;

use idlewarden_mod_install::Index;

const REGISTRY_INDEX: &str = "https://idlewarden.github.io/registry/index.json";
const LARGEST_INDEX: u64 = 8 * 1024 * 1024;
pub const LARGEST_ARCHIVE: u64 = 128 * 1024 * 1024;

pub fn index() -> Result<Index, String> {
    let listing = listing(std::env::var_os("IDLEWARDEN_REGISTRY_INDEX"))?;
    serde_json::from_slice(&listing)
        .map_err(|error| format!("the registry index is unreadable: {error}"))
}

fn listing(local: Option<OsString>) -> Result<Vec<u8>, String> {
    let Some(path) = local.map(PathBuf::from) else {
        return download(REGISTRY_INDEX, LARGEST_INDEX);
    };
    tracing::warn!(path = %path.display(), "reading the registry index from a local file");
    std::fs::read(&path).map_err(|error| {
        format!(
            "cannot read the registry index at {}: {error}",
            path.display()
        )
    })
}

pub fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(format!(
            "refusing to download over anything but https: {url}"
        ));
    }
    ureq::get(url)
        .call()
        .map_err(|error| format!("cannot download {url}: {error}"))?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| format!("cannot read {url}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("idlewarden-registry-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("folder");
        dir
    }

    #[test]
    fn a_local_index_is_read_from_disk_instead_of_the_registry() {
        let path = folder("local-index").join("index.json");
        std::fs::write(&path, br#"{"mods": []}"#).expect("index written");

        let read = listing(Some(path.into_os_string())).expect("reads the file");

        assert_eq!(read, br#"{"mods": []}"#);
    }

    #[test]
    fn a_local_index_that_is_missing_says_where_it_looked() {
        let path = folder("absent-index").join("index.json");

        let refused = listing(Some(path.clone().into_os_string())).unwrap_err();

        assert!(refused.contains(&path.display().to_string()), "{refused}");
    }

    #[test]
    fn a_download_that_is_not_https_never_leaves_the_machine() {
        let refused = download("http://example.com/mod.zip", LARGEST_ARCHIVE).unwrap_err();

        assert!(refused.contains("https"), "{refused}");
    }
}
