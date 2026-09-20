// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Loader {
    Bepinex,
    Melonloader,
    ReloadedIi,
    /// The game has a mod folder of its own, named by the entry: Cookie Clicker
    /// loads `resources/app/mods/local/<id>/` (ADR-0018).
    Game,
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
    #[error("`{0}` is not a mod folder inside the game")]
    BadModsPath(String),
    #[error("this mod needs the game's own mod folder, which the entry does not name")]
    NoModsPath,
}

impl Loader {
    pub fn label(self) -> &'static str {
        match self {
            Loader::Game => "the game's own loader",
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
        mods_path: Option<&str>,
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
            Loader::Game => {
                let mods = mods_path.ok_or(LoaderError::NoModsPath)?;
                if !is_inside_the_game(mods) {
                    return Err(LoaderError::BadModsPath(mods.to_owned()));
                }
                let root = game.join(mods);
                if !root.is_dir() {
                    return Err(missing());
                }
                Ok(Destination::Own(root.join(mod_id)))
            }
            Loader::Manual => Err(LoaderError::Manual),
        }
    }
}

fn is_inside_the_game(mods: &str) -> bool {
    !mods.is_empty()
        && Path::new(mods).is_relative()
        && Path::new(mods)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}
