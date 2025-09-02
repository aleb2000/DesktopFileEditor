use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(feature = "flatpak")]
use crate::flatpak;

#[cfg(not(feature = "flatpak"))]
pub fn display_path(path: &Path) -> PathBuf {
    // Standard implementation just copies the original path
    path.to_path_buf()
}

#[cfg(feature = "flatpak")]
pub fn display_path(path: &Path) -> PathBuf {
    flatpak::host_path(path)
}

#[cfg(not(feature = "flatpak"))]
pub fn binary_search_paths() -> Option<OsString> {
    std::env::var_os("PATH")
}

#[cfg(feature = "flatpak")]
pub fn binary_search_paths() -> Option<OsString> {
    flatpak::binary_search_paths()
}

#[cfg(not(feature = "flatpak"))]
pub fn application_paths() -> impl Iterator<Item = PathBuf> {
    freedesktop_desktop_entry::default_paths()
}

#[cfg(feature = "flatpak")]
pub fn application_paths() -> impl Iterator<Item = PathBuf> {
    flatpak::application_paths()
}
