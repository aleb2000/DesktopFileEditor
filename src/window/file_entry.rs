/*
* Copyright © 2025 Alessandro Balducci
*
* This file is part of Desktop File Editor.
* Desktop File Editor is free software: you can redistribute it and/or modify it under the terms of the
* GNU General Public License as published by the Free Software Foundation,
* either version 3 of the License, or (at your option) any later version.
* Desktop File Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
* without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
* See the GNU General Public License for more details.
* You should have received a copy of the GNU General Public License along with Desktop File Editor. If not, see <https://www.gnu.org/licenses/>.
*/

use std::{
    fs,
    path::{Path, PathBuf},
};

use freedesktop_desktop_entry::{DecodeError, DesktopEntry};
use gtk::{
    gio,
    glib::{self, subclass::types::ObjectSubclassIsExt, Object},
};

use crate::{desktop_file_view::desktop_entry_ext::NO_LOCALE, shellparse};

mod imp {
    use adw::prelude::ObjectExt;
    use gtk::glib::{self, Properties};
    use gtk::subclass::prelude::*;
    use std::cell::Cell;
    use std::{cell::RefCell, path::PathBuf};

    use gtk::{
        glib::object_subclass,
        subclass::prelude::{ObjectImpl, ObjectSubclass},
    };

    use super::{ShouldShow, ValidityStatus};

    #[derive(Default, Properties, Debug)]
    #[properties(wrapper_type = super::FileEntry)]
    pub struct FileEntry {
        #[property(get, set)]
        pub path: RefCell<PathBuf>,

        #[property(get, set, nullable)]
        pub name: RefCell<Option<String>>,

        #[property(get, set)]
        pub icon: RefCell<String>,

        #[property(get, set, builder(ShouldShow::default()))]
        pub should_show: Cell<ShouldShow>,

        #[property(get, set)]
        pub validity_status: RefCell<ValidityStatus>,
    }

    #[object_subclass]
    impl ObjectSubclass for FileEntry {
        const NAME: &'static str = "DMEntryObj";
        type Type = super::FileEntry;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FileEntry {}
}

glib::wrapper! {
    pub struct FileEntry(ObjectSubclass<imp::FileEntry>);
}

impl FileEntry {
    pub fn new(
        path: PathBuf,
        name: Option<String>,
        icon: String,
        should_show: ShouldShow,
        validity_status: ValidityStatus,
    ) -> Self {
        Object::builder()
            .property("path", path)
            .property("name", name)
            .property("icon", icon)
            .property("should_show", should_show)
            .property("validity_status", validity_status)
            .build()
    }

    pub fn from_path(path: &Path) -> Result<Self, DecodeError> {
        let path = fs::canonicalize(path).unwrap_or_else(|_| {
            panic!(
                "Failed to obtain canonical path for {}",
                path.to_string_lossy()
            )
        });

        let entry = DesktopEntry::from_path(path.clone(), Some(&NO_LOCALE))?;

        let name = entry.name(&NO_LOCALE).map(String::from);

        let icon = entry
            .icon()
            .map(|s| s.to_string())
            .unwrap_or_else(|| String::from(Self::DEFAULT_ICON));

        let no_display = entry.no_display();
        let hidden = entry
            .desktop_entry("Hidden")
            .map(|x| x == "true")
            .unwrap_or(false);

        Ok(FileEntry::new(
            path,
            name,
            icon,
            ShouldShow::new(no_display, hidden),
            ValidityStatus::from_desktop_entry(&entry),
        ))
    }

    pub fn update(&self) -> Result<(), DecodeError> {
        let updated = Self::from_path(&self.path())?;
        let updated_imp = updated.imp();
        let imp = self.imp();
        imp.name.swap(&updated_imp.name);
        imp.icon.swap(&updated_imp.icon);
        imp.should_show.swap(&updated_imp.should_show);
        imp.validity_status.swap(&updated_imp.validity_status);
        self.notify_name();
        self.notify_icon();
        self.notify_should_show();
        self.notify_validity_status();
        Ok(())
    }

    pub fn search_key(&self) -> String {
        format!(
            "{} {}",
            self.name().unwrap_or_default(),
            self.path().to_str().unwrap_or_default()
        )
    }
}

pub trait ToGIcon {
    const DEFAULT_ICON: &'static str = "application-x-executable";

    fn default_exec_gicon() -> gio::Icon {
        gio::Icon::for_string(Self::DEFAULT_ICON).unwrap()
    }

    fn icon_string(&self) -> Option<String>;

    fn gicon(&self) -> gio::Icon {
        gio::Icon::for_string(&self.icon_string().unwrap_or(Self::DEFAULT_ICON.to_string()))
            .unwrap_or_else(|_| Self::default_exec_gicon())
    }
}

impl ToGIcon for FileEntry {
    fn icon_string(&self) -> Option<String> {
        Some(self.icon())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "ShouldShow")]
pub enum ShouldShow {
    Yes,
    NoDisplay,
    Hidden,
    NoDisplayAndHidden,
}

impl ShouldShow {
    fn new(no_display: bool, hidden: bool) -> Self {
        match (no_display, hidden) {
            (true, true) => Self::NoDisplayAndHidden,
            (true, false) => Self::NoDisplay,
            (false, true) => Self::Hidden,
            (false, false) => Self::Yes,
        }
    }
}

impl Default for ShouldShow {
    fn default() -> Self {
        Self::Yes
    }
}

#[derive(Debug, Default, Clone, glib::Boxed)]
#[boxed_type(name = "ValidityStatus")]
pub struct ValidityStatus {
    empty_name: bool,
    exec_ok: bool,
    exec_fail_reason: Option<String>,
}

impl ValidityStatus {
    pub fn from_desktop_entry(entry: &DesktopEntry) -> ValidityStatus {
        let (exec_ok, exec_fail_reason) = match parse_exec(entry) {
            Ok(binary) => match which::which(binary) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            },
            Err(e) => match e {
                // ExecError::WrongFormat(s) => (false, Some(format!("Wrong Exec Format: {s}"))),
                // ExecError::ExecFieldIsEmpty => (false, Some("Exec field is empty".to_string())),
                ExecError::ExecParseError => (false, Some("Exec parse error".to_string())),

                #[cfg(feature = "steam")]
                ExecError::SteamAppNotInstalled => {
                    (false, Some("Steam app not installed".to_string()))
                }

                ExecError::ExecFieldNotFound => (true, None),
            },
        };

        assert!(
            (exec_ok && exec_fail_reason.is_none()) || (!exec_ok && exec_fail_reason.is_some())
        );

        ValidityStatus {
            empty_name: entry.name(&NO_LOCALE).is_none(),
            exec_ok,
            exec_fail_reason,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.empty_name && self.exec_ok
    }

    pub fn error_string(&self) -> Option<String> {
        if self.is_valid() {
            return None;
        }

        let mut s = String::new();
        if self.empty_name {
            s.push_str("Missing name field\n");
        }
        if !self.exec_ok {
            s.push_str(
                self.exec_fail_reason
                    .as_ref()
                    .expect("Failing exec field should have a reason"),
            );
            s.push('\n');
        }
        Some(s.trim().to_string())
    }
}

fn parse_exec(entry: &DesktopEntry) -> Result<String, ExecError> {
    let exec = entry.exec().ok_or(ExecError::ExecFieldNotFound)?;
    let mut command = shellparse::parse(exec).ok_or(ExecError::ExecParseError)?;

    #[cfg(feature = "steam")]
    if command.is_steam_app() && !command.is_steam_app_installed() {
        return Err(ExecError::SteamAppNotInstalled);
    }

    command.flatten_env();
    Ok(command.command)
}

enum ExecError {
    ExecFieldNotFound,
    ExecParseError,

    #[cfg(feature = "steam")]
    SteamAppNotInstalled,
}
