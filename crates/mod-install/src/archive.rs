// SPDX-License-Identifier: MPL-2.0
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::loader::Destination;

pub const LARGEST_EXTRACTED: u64 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("`{0}` is not a mod id this host will turn into a folder name")]
    InvalidId(String),
    #[error(
        "the download does not match the registry checksum (expected {expected}, got {actual})"
    )]
    Checksum { expected: String, actual: String },
    #[error("the archive could not be read: {0}")]
    Unreadable(#[from] zip::result::ZipError),
    #[error("the archive holds `{0}`, which would land outside the mod folder")]
    Escapes(String),
    #[error("the archive unpacks to more than {} MiB", LARGEST_EXTRACTED / 1024 / 1024)]
    TooLarge,
    #[error("the archive holds no files")]
    Empty,
    #[error("the mod could not be written: {0}")]
    Io(#[from] std::io::Error),
}

type Archive<'a> = ZipArchive<Cursor<&'a [u8]>>;

struct Planned {
    index: usize,
    relative: PathBuf,
    size: u64,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn install(
    archive: &[u8],
    sha256: &str,
    id: &str,
    destination: &Destination,
) -> Result<PathBuf, InstallError> {
    if !is_folder_safe(id) {
        return Err(InstallError::InvalidId(id.to_owned()));
    }

    let actual = sha256_hex(archive);
    if !actual.eq_ignore_ascii_case(sha256) {
        return Err(InstallError::Checksum {
            expected: sha256.to_owned(),
            actual,
        });
    }

    let mut zip = ZipArchive::new(Cursor::new(archive))?;
    let files = plan(&mut zip)?;

    let staging = staging(destination, id);
    let _ = fs::remove_dir_all(&staging);
    let written =
        unpack(&mut zip, &files, &staging).and_then(|()| place(&staging, &files, destination));
    let _ = fs::remove_dir_all(&staging);
    written?;

    Ok(destination.path().to_owned())
}

fn is_folder_safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

fn plan(zip: &mut Archive<'_>) -> Result<Vec<Planned>, InstallError> {
    let mut files = Vec::new();
    let mut total = 0u64;

    for index in 0..zip.len() {
        let file = zip.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let relative = file
            .enclosed_name()
            .filter(|_| !file.is_symlink())
            .ok_or_else(|| InstallError::Escapes(file.name().to_owned()))?;

        total = total.saturating_add(file.size());
        if total > LARGEST_EXTRACTED {
            return Err(InstallError::TooLarge);
        }
        files.push(Planned {
            index,
            relative,
            size: file.size(),
        });
    }

    if files.is_empty() {
        return Err(InstallError::Empty);
    }
    Ok(files)
}

fn staging(destination: &Destination, id: &str) -> PathBuf {
    let name = format!(".{id}.partial");
    match destination {
        Destination::Own(dir) => dir.with_file_name(name),
        Destination::Shared(dir) => dir.join(name),
    }
}

fn unpack(zip: &mut Archive<'_>, files: &[Planned], staging: &Path) -> Result<(), InstallError> {
    for planned in files {
        let target = staging.join(&planned.relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let entry = zip.by_index(planned.index)?;
        let mut out = fs::File::create(&target)?;
        let copied = std::io::copy(&mut entry.take(planned.size + 1), &mut out)?;
        if copied > planned.size {
            return Err(InstallError::TooLarge);
        }
    }
    Ok(())
}

fn place(staging: &Path, files: &[Planned], destination: &Destination) -> Result<(), InstallError> {
    match destination {
        Destination::Own(dir) => {
            if dir.exists() {
                fs::remove_dir_all(dir)?;
            }
            fs::rename(staging, dir)?;
        }
        Destination::Shared(dir) => {
            for planned in files {
                let target = dir.join(&planned.relative);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(staging.join(&planned.relative), target)?;
            }
        }
    }
    Ok(())
}
