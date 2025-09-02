use once_cell::sync::Lazy;
use std::ffi::CStr;
use std::fs::File;
use std::os::fd::AsFd;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};
use zbus::blocking::Connection;

use crate::util::DocumentsInterfaceProxyBlocking;

static DBUS_SESSION_CONNECTION_BLOCKING: Lazy<Connection> =
    Lazy::new(|| Connection::session().expect("Failed to connect to session DBus"));

pub static DOCUMENTS_PROXY_BLOCKING: Lazy<DocumentsInterfaceProxyBlocking> = Lazy::new(|| {
    DocumentsInterfaceProxyBlocking::new(&DBUS_SESSION_CONNECTION_BLOCKING)
        .expect("Failed to create Documents interface proxy")
});

// In a flatpak environment we can't access host directories dynamically based on the XDG_DATA_DIRS
// varaible, hence we hardcode the directories here. The flatpak container must also be set up with
// appropriate holes in the sandboxing so that these directories are accessible.

const DATA_DIRS: [&str; 5] = [
    // Common value of XDG_DATA_HOME
    ".local/share",
    // Common values for XDG_DATA_DIRS
    ".local/share/flatpak/exports/share",
    "/var/lib/flatpak/exports/share",
    // The host's /usr directory is not available inside a flatpak container.
    // It can be made available, but it will be mounted at /run/host/usr
    "/run/host/usr/local/share/",
    "/run/host/usr/share/",
];

/// Flatpak initialization code
pub fn init() {
    // To make sure we properly look up icons we need to update the XDG_DATA_DIRS env var
    let mut xdg_data_dirs = env::var("XDG_DATA_DIRS").unwrap_or_else(|_| String::new());

    for dir in DATA_DIRS {
        if xdg_data_dirs
            .split(":")
            .all(|existing_dir| existing_dir != dir)
        {
            xdg_data_dirs.push(':');
            xdg_data_dirs.push_str(dir);
        }
    }

    env::set_var("XDG_DATA_DIRS", xdg_data_dirs);
}

// pub fn is_container() -> bool {
//     std::env::var("container").is_ok()
// }

pub fn application_paths() -> impl Iterator<Item = PathBuf> {
    DATA_DIRS.into_iter().map(|dir| {
        if !dir.starts_with("/") {
            // Local (home relative) path
            let home = std::env::home_dir().expect("No home? we can't work like this");
            home.join(dir)
        } else {
            // Absolute path
            PathBuf::from(dir)
        }
        .join("applications")
    })
}

pub fn binary_search_paths() -> Option<OsString> {
    let mut path = env::var_os("PATH");
    if let Some(ref mut path) = path {
        path.push(":/run/host/bin:/run/host/usr/bin");
    }

    path
}

pub fn host_path(path: &Path) -> PathBuf {
    if path.as_os_str().is_empty() {
        return path.to_path_buf();
    }

    if let Ok(file) = File::open(path) {
        if let Ok(fd) = file.as_fd().try_clone_to_owned() {
            let proxy = &DOCUMENTS_PROXY_BLOCKING;

            let doc_id = proxy.add(fd.into(), true, false).unwrap();
            let host_paths_map = proxy.get_host_paths(&[&doc_id]).unwrap();
            let host_path_bytes = host_paths_map[&doc_id].clone();
            let host_path = CStr::from_bytes_with_nul(&host_path_bytes)
                .unwrap()
                .to_str()
                .expect("Host path does not contain valid UTF-8");
            return PathBuf::from(host_path);
        }
    }

    path.to_path_buf()
}
