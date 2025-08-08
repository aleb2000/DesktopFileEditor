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
    cmp::Ordering,
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    fmt::Write,
};

use freedesktop_desktop_entry::{DesktopEntry, GroupName, Key, LocaleMap, Value};

use crate::window::file_entry::ToGIcon;

pub const NO_LOCALE: [&str; 0] = [];
pub const DEFAULT_LOCALE: &str = "Default";

const GROUPS_ORDER: [&str; 1] = ["Desktop Entry"];

/// Defines the order the keymap entries will be displayed in.
const KEYMAP_ORDER: [&str; 25] = [
    "Name",
    "GenericName",
    "Comment",
    "Icon",
    "DBusActivatable",
    "Exec",
    "TryExec",
    "Categories",
    "Keywords",
    "Terminal",
    "MimeType",
    "PrefersNonDefaultGPU",
    "StartupNotify",
    "StartupWMClass",
    "Path",
    "Type",
    "URL",
    "SingleMainWindow",
    "NoDisplay",
    "Hidden",
    "OnlyShowIn",
    "NotShowIn",
    "Implements",
    "Version",
    "Actions",
];

fn fixed_order_comparator(fixed_order: &[&str], a: &str, b: &str) -> Ordering {
    let a_fixed_order = fixed_order.iter().position(|key| *key == a);
    let b_fixed_order = fixed_order.iter().position(|key| *key == b);
    match (a_fixed_order, b_fixed_order) {
        (Some(a_pos), Some(b_pos)) => a_pos.cmp(&b_pos),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

pub type VecKeyMap<'a> = Vec<(Key, (Value, LocaleMap))>;

pub trait DesktopEntryExt {
    fn sorted_keymap(&self, group: &str) -> Option<VecKeyMap>;
    fn sorted_groups(&self) -> Vec<(GroupName, VecKeyMap)>;
    fn locales(&self) -> Vec<String>;

    /// Convert the desktop entry to a `String`, with entries sorted by key, following the
    /// `KEYMAP_ORDER` fixed priority list. If a key is not in the defined fixed order they will be
    /// written alphabetically after the items present in the array. Groups are sorted the same way following
    /// `GROUPS_ORDER`.
    ///
    /// This function also adds the `X-Ubuntu-Gettext-Domain` entry, which is removed by
    /// the `DesktopEntry` decoder.
    fn to_sorted_entry_string(&self) -> String {
        let mut result = String::new();

        // Code adapted from Display implementation of DesktopEntry
        for (group_name, keymap) in self.sorted_groups() {
            let _ = writeln!(&mut result, "[{group_name}]");

            for (key, (value, localizations)) in keymap {
                let _ = writeln!(&mut result, "{key}={value}");
                for (locale, localized) in localizations {
                    let _ = writeln!(&mut result, "{key}[{locale}]={localized}");
                }
            }
            let _ = writeln!(&mut result);
        }

        result
    }

    fn entry(&self, group_name: &str, key: &str, locale: Option<&str>) -> Option<&str>;
    fn add_entry(&mut self, group_name: String, key: String) -> bool;
    fn add_group(&mut self, name: String);
    fn remove_group(&mut self, name: String);

    fn add_action(&mut self, name: &str) {
        self.add_group(format!("Desktop Action {name}"));
    }

    /*fn remove_action(&mut self, name: &str) {
        self.remove_group(format!("Desktop Action {name}"));
    }*/

    fn remove_entry(&mut self, group: String, key: String);
}

impl DesktopEntryExt for DesktopEntry {
    fn sorted_keymap(&self, group_name: &str) -> Option<VecKeyMap> {
        let keymap = self.groups.group(group_name)?.0.clone();
        let mut keymap = Vec::from_iter(keymap);

        // Here we can add the X-Ubuntu-Gettext-Domain entry if it exists
        if group_name == "Desktop Entry" && self.ubuntu_gettext_domain.is_some() {
            let ubuntu_gettext_domain_key = String::from("X-Ubuntu-Gettext-Domain");
            let ubuntu_gettext_domain = self.ubuntu_gettext_domain.clone().unwrap();
            let ubuntu_gettext_domain_value = (ubuntu_gettext_domain, BTreeMap::new());

            keymap.push((ubuntu_gettext_domain_key, ubuntu_gettext_domain_value));
        }

        keymap.sort_by(|(a, _), (b, _)| fixed_order_comparator(&KEYMAP_ORDER, a, b));
        Some(keymap)
    }

    fn sorted_groups(&self) -> Vec<(GroupName, VecKeyMap)> {
        let mut groups = Vec::new();
        for group in self.groups.0.keys() {
            let vec_keymap = self.sorted_keymap(group).unwrap();
            groups.push((group.clone(), vec_keymap))
        }

        groups.sort_by(|(a, _), (b, _)| fixed_order_comparator(&GROUPS_ORDER, a, b));
        groups
    }

    fn locales(&self) -> Vec<String> {
        self.groups.0
            .iter()
            .flat_map(|(_, group)| group.0.values())
            .flat_map(|(_default, localized)| localized.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn entry(&self, group_name: &str, key: &str, locale: Option<&str>) -> Option<&str> {
        let keymap = self.groups.group(group_name);
        if let Some(group) = keymap {
            if let Some((value, localized_values)) = group.0.get(key) {
                match locale {
                    Some(locale) => {
                        if let Some(localized_value) = localized_values.get(locale) {
                            return Some(localized_value);
                        }
                    }
                    None => return Some(value),
                }
            }
        };
        None
    }

    fn add_entry(&mut self, group_name: String, key: String) -> bool {
        self.groups.0
            .get_mut(&group_name)
            .map(move |group| {
                let key_exists = group.entry(&key[..]).is_some();
                group.0.entry(key).or_default();
                !key_exists
            })
            .unwrap_or(false)
    }

    fn add_group(&mut self, name: String) {
        self.groups.0.entry(name).or_default();
    }

    fn remove_group(&mut self, name: String) {
        self.groups.0.remove(&name);
    }

    fn remove_entry(&mut self, group_name: String, key: String) {
        self.groups.0.entry(group_name).and_modify(|group| {
            if let Entry::Occupied(entry) = group.0.entry(key) {
                entry.remove();
            }
        });
    }
}

impl ToGIcon for DesktopEntry {
    fn icon_string(&self) -> Option<String> {
        self.icon().map(|icon| icon.to_string())
    }
}
