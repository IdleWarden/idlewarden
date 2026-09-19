// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Loader {
    Bepinex,
    Melonloader,
    ReloadedIi,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Own(PathBuf),
    Shared(PathBuf),
}

impl Destination {
    pub fn path(&self) -> &Path {
        match self {
            Destination::Own(path) | Destination::Shared(path) => path,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LoaderError {
    #[error("{loader} is not installed in {}; install it in the game first", .game.display())]
    Missing { loader: &'static str, game: PathBuf },
    #[error("Reloaded-II is not installed, so its mods folder is unknown")]
    ReloadedMissing,
    #[error("this mod is installed by hand, following its own instructions")]
    Manual,
}

impl Loader {
    pub fn label(self) -> &'static str {
        match self {
            Loader::Bepinex => "BepInEx",
            Loader::Melonloader => "MelonLoader",
            Loader::ReloadedIi => "Reloaded-II",
            Loader::Manual => "manual",
        }
    }

    pub fn destination(
        self,
        game: &Path,
        reloaded_mods: Option<&Path>,
        mod_id: &str,
    ) -> Result<Destination, LoaderError> {
        let missing = || LoaderError::Missing {
            loader: self.label(),
            game: game.to_owned(),
        };

        match self {
            Loader::Bepinex => {
                let root = game.join("BepInEx");
                if !root.join("core").is_dir() {
                    return Err(missing());
                }
                Ok(Destination::Own(root.join("plugins").join(mod_id)))
            }
            Loader::Melonloader => {
                if !game.join("MelonLoader").is_dir() {
                    return Err(missing());
                }
                Ok(Destination::Shared(game.join("Mods")))
            }
            Loader::ReloadedIi => reloaded_mods
                .filter(|mods| mods.is_dir())
                .map(|mods| Destination::Own(mods.join(mod_id)))
                .ok_or(LoaderError::ReloadedMissing),
            Loader::Manual => Err(LoaderError::Manual),
        }
    }
}
