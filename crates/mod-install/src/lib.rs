// SPDX-License-Identifier: MPL-2.0
mod archive;
mod index;
mod loader;

#[cfg(test)]
mod tests;

pub use archive::{install, sha256_hex, InstallError, LARGEST_EXTRACTED};
pub use index::{Bridge, Index, ModEntry, ModVersion, Release};
pub use loader::{Destination, Loader, LoaderError};
