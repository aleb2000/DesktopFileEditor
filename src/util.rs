use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use zbus::proxy;

#[cfg(not(feature = "flatpak"))]
pub fn display_path(path: &Path) -> PathBuf {
    // Standard implementation just copies the original path
    path.to_path_buf()
}

#[cfg(feature = "flatpak")]
mod flatpak {
    use once_cell::sync::Lazy;
    use zbus::blocking::Connection;

    use crate::util::DocumentsInterfaceProxyBlocking;

    static DBUS_SESSION_CONNECTION_BLOCKING: Lazy<Connection> =
        Lazy::new(|| Connection::session().expect("Failed to connect to session DBus"));

    pub static DOCUMENTS_PROXY_BLOCKING: Lazy<DocumentsInterfaceProxyBlocking> = Lazy::new(|| {
        DocumentsInterfaceProxyBlocking::new(&DBUS_SESSION_CONNECTION_BLOCKING)
            .expect("Failed to create Documents interface proxy")
    });
}

#[cfg(feature = "flatpak")]
pub fn display_path(path: &Path) -> PathBuf {
    use std::ffi::CStr;
    use std::fs::File;
    use std::os::fd::AsFd;

    if path.as_os_str().is_empty() {
        return path.to_path_buf();
    }

    if let Ok(file) = File::open(path) {
        if let Ok(fd) = file.as_fd().try_clone_to_owned() {
            let proxy = &flatpak::DOCUMENTS_PROXY_BLOCKING;

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

#[proxy(
    interface = "org.freedesktop.portal.Documents",
    default_service = "org.freedesktop.portal.Documents",
    default_path = "/org/freedesktop/portal/documents"
)]
trait DocumentsInterface {
    fn add(
        &self,
        o_path_fd: zbus::zvariant::OwnedFd,
        reuse_existing: bool,
        persistent: bool,
    ) -> zbus::Result<String>;

    fn get_host_paths(&self, doc_ids: &[&str]) -> zbus::Result<HashMap<String, Vec<u8>>>;
}
