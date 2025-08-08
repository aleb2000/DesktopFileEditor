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

mod icon_entry_row;
mod new_entry_dialog;
mod tagged_entry_row;
mod util;

use std::borrow::Borrow;
use std::collections::btree_map::Entry;

use adw::{prelude::*, SwitchRow};
use gtk::gio::Cancellable;
use gtk::glib::clone::Downgrade;
use gtk::glib::{self, property::PropertySet, subclass::types::ObjectSubclassIsExt};
use gtk::glib::{clone, closure_local, SignalHandlerId};
use icon_entry_row::IconEntryRow;
use once_cell::sync::Lazy;
use regex::Regex;
use tagged_entry_row::TaggedEntryRow;
use util::{add_additional_options_buttons, make_additional_options_menu};

use crate::desktop_file_view::desktop_entry_ext::DesktopEntryExt;
use crate::desktop_file_view::imp::DesktopEntryCell;
use crate::desktop_file_view::string_entry_row::StringEntryRow;
use crate::window::file_entry::ToGIcon;

use super::DesktopFileView;

static DESKTOP_ACTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^Desktop Action (.+)$").expect("Failed to compile regex"));

mod imp {
    use crate::desktop_file_view::known_entries::KEYS_DESCRIPTIONS;
    use crate::desktop_file_view::string_entry_row::StringEntryRow;
    use std::borrow::Borrow;
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use adw::{prelude::*, AlertDialog};

    use gtk::gio::Cancellable;
    use gtk::glib::clone;
    use gtk::glib::clone::Downgrade;
    use gtk::glib::subclass::InitializingObject;
    use gtk::glib::Properties;
    use gtk::{glib, template_callbacks, CompositeTemplate};

    use crate::desktop_file_view::imp::DesktopEntryCell;
    use crate::desktop_file_view::DesktopFileView;

    use super::new_entry_dialog::NewEntryDialog;
    use super::tagged_entry_row::TaggedEntryRow;
    use super::RowWidgetExt;

    #[derive(CompositeTemplate, Default, Properties)]
    #[template(resource = "/org/argoware/desktop_file_editor/desktop_file_group.ui")]
    #[properties(wrapper_type = super::DesktopFileGroup)]
    pub struct DesktopFileGroup {
        #[template_child]
        pub entry_list: TemplateChild<gtk::ListBox>,

        #[template_child]
        pub new_entry_btn: TemplateChild<adw::ButtonRow>,

        #[template_child]
        pub edit_dialog: TemplateChild<adw::AlertDialog>,

        #[template_child]
        pub edit_dialog_entry: TemplateChild<gtk::Entry>,

        #[property(get, set = DesktopFileGroup::set_name)]
        name: RefCell<String>,

        #[property(get, set, construct, default = true)]
        show_group_name: Cell<bool>,

        pub desktop_file_view: RefCell<Option<<DesktopFileView as Downgrade>::Weak>>,
        pub localized_widgets: RefCell<Vec<LocalizedWidget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DesktopFileGroup {
        const NAME: &'static str = "DesktopFileGroup";
        type Type = super::DesktopFileGroup;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action(
                "desktop_file_group.show_entry_info",
                Some(&String::static_variant_type()),
                |group, _action, args| {
                    let variant = args.expect("Missing action parameter");
                    if let Some(key) = String::from_variant(variant) {
                        let info = KEYS_DESCRIPTIONS[&key[..]];
                        let dialog = AlertDialog::builder().heading(&key).body(info).build();

                        dialog.add_response("close", "Close");
                        dialog.present(Some(group));
                    }
                },
            );

            klass.install_action(
                "desktop_file_group.remove",
                None,
                |group, _action, _args| {
                    let desktop_file_view = group
                        .imp()
                        .desktop_file_view
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .upgrade();
                    if let Some(desktop_file_view) = desktop_file_view {
                        desktop_file_view.remove_group(group);
                    }
                },
            );

            klass.install_action(
                "desktop_file_group.remove_entry",
                Some(&String::static_variant_type()),
                |group, _action, args| {
                    let variant = args.expect("Missing action parameter");
                    if let Some(key) = String::from_variant(variant) {
                        // Find the key of the row that will grab focus after this row is deleted,
                        // if any
                        let next_focus_key = if let Some(row) = group.find_entry_widget(&key) {
                            let target_widget = match (row.prev_sibling(), row.next_sibling()) {
                                (_, Some(next)) if next != *group.imp().new_entry_btn => Some(next),
                                (Some(prev), Some(next)) if next == *group.imp().new_entry_btn => {
                                    Some(prev)
                                }
                                (Some(prev), None) => Some(prev),
                                _ => {
                                    println!("None");
                                    None
                                }
                            };
                            target_widget
                                .and_downcast::<adw::PreferencesRow>()
                                .map(|row| row.title())
                        } else {
                            None
                        };

                        // Remove row
                        group.remove_entry(key);

                        // Next row grabs focus
                        if let Some(next_focus_key) = next_focus_key {
                            println!("Next focus key: {next_focus_key}");
                            group.find_entry_widget(&next_focus_key).map(|row| {
                                glib::idle_add_local_once(move || {
                                    row.grab_focus();
                                })
                            });
                        }
                    }
                },
            );

            klass.install_action("desktop_file_group.edit", None, |group, _action, _args| {
                group.show_edit_dialog();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DesktopFileGroup {
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for DesktopFileGroup {}
    impl BoxImpl for DesktopFileGroup {}

    #[template_callbacks]
    impl DesktopFileGroup {
        #[template_callback]
        fn on_new_entry_btn_activated(&self, btn: adw::ButtonRow) {
            let desktop_file_view = self
                .desktop_file_view
                .borrow()
                .clone()
                .expect("Cannot create new entry dialog without DesktopFileView reference");

            let dialog = NewEntryDialog::new(self.obj().name(), desktop_file_view);
            dialog.clone().choose(
                &btn,
                Cancellable::NONE,
                clone!(
                    #[weak(rename_to=this)]
                    self,
                    #[weak]
                    dialog,
                    move |response| {
                        if response == "add" {
                            match dialog.validated_key() {
                                Ok(key) => {
                                    this.obj().add_entry(&key);
                                }

                                Err(e) => eprintln!(
                                    "Chosen key cannot be added, this is likely a bug: {e:?}"
                                ),
                            }
                        }
                    }
                ),
            );
        }

        #[template_callback]
        fn on_edit_dialog_entry_changed(&self, entry: gtk::Entry) {
            let dialog = self.edit_dialog.clone();
            dialog.set_response_enabled("edit", !entry.text().trim().is_empty());
        }

        #[template_callback]
        fn on_edit_dialog_entry_activate(&self, entry: gtk::Entry) {
            if entry.text().trim().is_empty() {
                return;
            }

            let dialog = self.edit_dialog.clone();
            dialog.set_close_response("edit");
            let closed = dialog.close();
            if !closed {
                eprintln!(
                    "Failed to close group edit dialog, closing forcefully, please report this bug!"
                );
                dialog.force_close();
            }
            dialog.set_close_response("close");
        }

        pub fn set_name(&self, name: &str) {
            if &self.name.borrow()[..] == name {
                return;
            }

            let old_name = self.name.replace(name.to_string());
            let obj = self.obj();
            obj.notify_name();

            // Automatically modify the desktop entry state to be in sync with the group name
            if let Some(desktop_file_view) = self.desktop_file_view.borrow().as_ref() {
                if let Some(desktop_file_view) = desktop_file_view.upgrade() {
                    let desktop_entry_rc = &desktop_file_view.desktop_entry();
                    let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
                    let mut desktop_entry = desktop_entry_cell.borrow_mut();

                    let value = desktop_entry.groups.0.remove(&old_name);
                    if let Some(value) = value {
                        desktop_entry.groups.0.insert(name.to_string(), value);
                    }
                    desktop_file_view.set_content_changed(true);
                }
            }
        }
    }

    pub enum LocalizedWidget {
        StringEntry(StringEntryRow),
        StringList(TaggedEntryRow),
    }

    impl LocalizedWidget {
        pub fn change_locale(&self, locale: Option<String>, localized_value: &str) {
            match self {
                LocalizedWidget::StringEntry(string_entry_row) => {
                    string_entry_row.change_locale(locale, localized_value)
                }
                LocalizedWidget::StringList(tagged_entry_row) => {
                    tagged_entry_row.change_locale(locale, localized_value)
                }
            }
        }

        pub fn entry_key(&self) -> String {
            match self {
                LocalizedWidget::StringEntry(string_entry_row) => string_entry_row.entry_key(),
                LocalizedWidget::StringList(tagged_entry_row) => tagged_entry_row.entry_key(),
            }
        }
    }

    impl From<StringEntryRow> for LocalizedWidget {
        fn from(value: StringEntryRow) -> Self {
            Self::StringEntry(value)
        }
    }

    impl From<TaggedEntryRow> for LocalizedWidget {
        fn from(value: TaggedEntryRow) -> Self {
            Self::StringList(value)
        }
    }
}

glib::wrapper! {
    pub struct DesktopFileGroup(ObjectSubclass<imp::DesktopFileGroup>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl DesktopFileGroup {
    pub fn new(name: &str, show_group_name: bool) -> Self {
        glib::Object::builder()
            .property("name", name)
            .property("show-group-name", show_group_name)
            .build()
    }

    pub fn populate(&self) {
        let imp = self.imp();

        // Clear current population
        imp.entry_list.remove_all();
        imp.localized_widgets.borrow_mut().clear();

        // Repopulate
        let desktop_file_view = imp.desktop_file_view.borrow().as_ref().unwrap().upgrade();
        if let Some(desktop_file_view) = desktop_file_view {
            {
                // Desktop Entry borrow should not escape block
                let desktop_entry_rc = &desktop_file_view.desktop_entry();
                let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
                let desktop_entry = desktop_entry_cell.borrow();

                if let Some(keymap) = desktop_entry.sorted_keymap(&self.name()) {
                    for (key, val) in keymap.iter() {
                        let val = &val.0;
                        let entry_widget = self.make_entry_widget(key, val);
                        imp.entry_list.append(&entry_widget);
                    }
                }
            }

            self.change_locale(desktop_file_view.locale().as_deref());
        }

        imp.entry_list.append(&imp.new_entry_btn.clone());
    }

    pub fn set_desktop_file_view(
        &self,
        desktop_file_view: Option<<DesktopFileView as Downgrade>::Weak>,
    ) {
        self.imp().desktop_file_view.set(desktop_file_view);
    }

    pub fn change_locale(&self, locale: Option<&str>) {
        let imp = self.imp();
        let desktop_file_view = imp.desktop_file_view.borrow().as_ref().unwrap().upgrade();
        if let Some(desktop_file_view) = desktop_file_view {
            for widget in imp.localized_widgets.borrow().iter() {
                let value = {
                    let desktop_entry_rc = &desktop_file_view.desktop_entry();
                    let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
                    let desktop_entry = desktop_entry_cell.borrow();

                    let entry_key = widget.entry_key();

                    desktop_entry
                        .entry(&self.name(), &entry_key, locale)
                        .unwrap_or_default()
                        .to_string()
                };
                widget.change_locale(locale.map(|s| s.to_owned()), &value);
            }
        }
    }

    pub fn add_entry(&self, key: &str) {
        let imp = self.imp();
        let desktop_file_view = imp.desktop_file_view.borrow().as_ref().unwrap().upgrade();
        // Try to add the entry to the Desktop Entry object state
        let added = if let Some(desktop_file_view) = desktop_file_view {
            let desktop_entry_rc = &desktop_file_view.desktop_entry();
            let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
            let mut desktop_entry = desktop_entry_cell.borrow_mut();

            let added = desktop_entry.add_entry(self.name(), key.to_string());
            desktop_file_view.set_content_changed(added);
            added
        } else {
            false
        };

        if added {
            // Set the default value for the appropriate type
            let widget_type = &EntryWidgetType::from_entry_key(key);
            let default_value = widget_type.default_value();
            self.set_entry_value(key.to_string(), default_value.to_string(), None);

            // Update the UI
            self.populate();

            // Focus newly added entry
            if let Some(row) = self.find_entry_widget(key) {
                glib::idle_add_local_once(move || {
                    row.grab_focus();
                });
            }
        }
    }

    fn find_entry_widget(&self, key: &str) -> Option<adw::PreferencesRow> {
        let imp = self.imp();
        let mut child = imp.entry_list.first_child();
        while child.is_some() {
            let widget = child.unwrap();
            if let Ok(row) = widget.clone().downcast::<adw::PreferencesRow>() {
                if row.title() == key {
                    return Some(row);
                }
            }
            child = widget.next_sibling();
        }
        None
    }

    fn show_edit_dialog(&self) {
        let imp = self.imp();
        let dialog = imp.edit_dialog.clone();
        let entry = imp.edit_dialog_entry.clone();
        entry.set_text(&self.name());

        if let Some(captures) = DESKTOP_ACTION_RE.captures(&entry.text()) {
            if let Some(m) = captures.get(1) {
                entry.select_region(m.start() as i32, m.end() as i32);
            }
        }
        let correct_selection_bounds = entry.selection_bounds();

        // The following is a very hacky way to make sure the text selection bounds are correct
        // This is necessary because for some reason the AlertDialog calls `grab_focus()`` on our
        // entry, which overwrites the selected region, and I could not find a way to run the
        // select region after this focus change except like this.
        // Yes, I am using the css class selector as a flag, yes it is ugly and unreliable, yes it
        // works
        entry.remove_css_class("selection_fixed");
        let selection_bound_notify = entry.connect_selection_bound_notify(move |entry| {
            if entry.has_css_class("selection_fixed") {
                return;
            }

            let selection_bounds = entry.selection_bounds();
            if selection_bounds != correct_selection_bounds {
                if let Some((start, end)) = correct_selection_bounds {
                    entry.select_region(start, end);
                    entry.add_css_class("selection_fixed");
                }
            }
        });

        dialog.choose(
            self,
            Cancellable::NONE,
            clone!(
                #[weak]
                entry,
                #[weak(rename_to=this)]
                self,
                move |response| {
                    entry.disconnect(selection_bound_notify);
                    if response == "edit" && !entry.text().trim().is_empty() {
                        this.set_name(entry.text().trim());
                    }
                }
            ),
        );

        // The grab focus must be here
        entry.grab_focus();
        entry.remove_css_class("selection_fixed");
    }

    fn remove_entry(&self, key: String) {
        let desktop_file_view = self
            .imp()
            .desktop_file_view
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade();
        if let Some(desktop_file_view) = desktop_file_view {
            let desktop_entry_rc = &desktop_file_view.desktop_entry();
            let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
            let mut desktop_entry = desktop_entry_cell.borrow_mut();

            desktop_entry.remove_entry(self.name(), key);
            desktop_file_view.set_content_changed(true);
        }

        self.populate()
    }

    /// Creates a widget suitable for the given key-value pair
    fn make_entry_widget(&self, key: &str, value: &str) -> gtk::Widget {
        let mut widget_type = EntryWidgetType::from_entry_key(key);
        if matches!(widget_type, EntryWidgetType::Unknown) {
            widget_type = EntryWidgetType::from_entry_value(value);
        }

        match widget_type {
            EntryWidgetType::BoolSwitch => self.bool_switch_widget(key, value),
            EntryWidgetType::IconEntry => self.icon_entry_widget(key, value),
            EntryWidgetType::StringEntry => self.string_entry_widget(key, value, false),
            EntryWidgetType::StringList => self.string_list_widget(key, value, false),
            EntryWidgetType::LocalizedStringList => self.string_list_widget(key, value, true),
            EntryWidgetType::LocalizedStringEntry | EntryWidgetType::Unknown => {
                self.string_entry_widget(key, value, true)
            }
        }
    }

    fn add_state_change_listener<T: RowWidgetExt + IsA<gtk::Widget>>(&self, widget: &T) {
        widget.connect_changed(clone!(
            #[weak(rename_to=this)]
            self,
            move |entry_row| {
                let key = entry_row.entry_key();
                let value = entry_row.entry_value();
                let locale = entry_row.entry_locale();
                this.set_entry_value(key, value, locale);
            }
        ));
    }

    /// Set the value of an entry in the backing Desktop Entry object state,
    /// does not actually update the widget text
    fn set_entry_value(&self, key: String, value: String, locale: Option<String>) {
        let desktop_file_view = self
            .imp()
            .desktop_file_view
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade();
        if let Some(desktop_file_view) = desktop_file_view {
            let desktop_entry_rc = &desktop_file_view.desktop_entry();
            let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
            let mut desktop_entry = desktop_entry_cell.borrow_mut();

            let group_name = self.name();

            if key == "X-Ubuntu-Gettext-Domain" && group_name == "Desktop Entry" {
                desktop_entry.ubuntu_gettext_domain = Some(value);
                return;
            }

            desktop_entry
                .groups
                .0
                .entry(group_name)
                .and_modify(|group| {
                    group
                        .0
                        .entry(key)
                        .and_modify(|(unlocalized_value, localized_values)| {
                            let original_value = match locale {
                                Some(ref locale) => {
                                    localized_values.entry(locale.clone()).or_default()
                                }
                                None => unlocalized_value,
                            };

                            let desktop_file_view = self
                                .imp()
                                .desktop_file_view
                                .borrow()
                                .as_ref()
                                .expect("Desktop File View not set")
                                .upgrade();
                            if let Some(desktop_file_view) = desktop_file_view {
                                if !desktop_file_view.content_changed() && *original_value != value
                                {
                                    desktop_file_view.set_content_changed(true);
                                }
                            }

                            *original_value = value;

                            // If after the change the entry is empty, we can remove it
                            // altogether
                            if let Some(locale) = locale {
                                if let Entry::Occupied(entry) =
                                    localized_values.entry(locale)
                                {
                                    if entry.get().is_empty() {
                                        entry.remove();
                                    }
                                }
                            }
                        });

                    // If after the change the entry is empty, we can remove it
                    // altogether
                    /*
                    if let Entry::Occupied(entry) = keymap.entry(key.into()) {
                        let (value, localized_values) = entry.get();
                        if value.is_empty() && localized_values.is_empty() {
                            entry.remove();
                        }
                    }*/
                });
        }
    }

    fn string_entry_widget(&self, key: &str, value: &str, localizable: bool) -> gtk::Widget {
        let entry_row = StringEntryRow::with_default_locale(localizable);
        entry_row.set_title(key);
        entry_row.set_text(value);

        self.add_state_change_listener(&entry_row);
        if localizable {
            self.imp()
                .localized_widgets
                .borrow_mut()
                .push(entry_row.clone().into());
        }

        entry_row.add_suffix(&make_additional_options_menu(key));

        entry_row.into()
    }

    fn bool_switch_widget(&self, key: &str, value: &str) -> gtk::Widget {
        if value != "true" && value != "false" {
            // FIXME: How to handle this?
            println!("Invalid switch active value!");
        }

        let switch_row = adw::SwitchRow::builder()
            .title(key)
            .active(value == "true")
            .build();

        switch_row.add_suffix(&make_additional_options_menu(key));

        self.add_state_change_listener(&switch_row);
        switch_row.into()
    }

    fn icon_entry_widget(&self, key: &str, value: &str) -> gtk::Widget {
        let icon_entry_row = IconEntryRow::new(key, value);
        icon_entry_row.update_icon();

        // Find a way to access parent image
        icon_entry_row.connect_activate(clone!(
            #[weak(rename_to=this)]
            self,
            move |icon_entry_row| {
                let _ = this
                    .imp()
                    .desktop_file_view
                    .borrow()
                    .as_ref()
                    .expect("Desktop File View not set")
                    .upgrade()
                    .map(|desktop_file_view| {
                        desktop_file_view.set_image_icon(&icon_entry_row.gicon());
                    });
            }
        ));

        self.add_state_change_listener(&icon_entry_row);
        icon_entry_row.into()
    }

    fn string_list_widget(&self, key: &str, value: &str, localizable: bool) -> gtk::Widget {
        let tagged_entry_row = TaggedEntryRow::from_string_list(localizable, None, value);
        tagged_entry_row.set_title(key);

        if localizable {
            self.imp()
                .localized_widgets
                .borrow_mut()
                .push(tagged_entry_row.clone().into());
        }

        // tagged_entry_row.add_suffix(&make_additional_options_menu(key));
        add_additional_options_buttons(
            key,
            clone!(
                #[weak]
                tagged_entry_row,
                move |button| {
                    tagged_entry_row.add_suffix(button);
                }
            ),
        );
        self.add_state_change_listener(&tagged_entry_row);
        tagged_entry_row.into()
    }
}

enum EntryWidgetType {
    StringEntry,
    LocalizedStringEntry,
    BoolSwitch,
    IconEntry,
    StringList,
    LocalizedStringList,
    Unknown,
}

static STRING_LIST_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("([A-Za-z0-9]*;)+").expect("Failed to compile regex"));

impl EntryWidgetType {
    pub fn from_entry_key(key: &str) -> Self {
        match key {
            "NoDisplay"
            | "Hidden"
            | "DBusActivatable"
            | "Terminal"
            | "StartupNotify"
            | "PrefersNonDefaultGPU"
            | "SingleMainWindow" => Self::BoolSwitch,
            "Name" | "GenericName" | "Comment" | "StartupWMClass" | "URL" => {
                Self::LocalizedStringEntry
            }
            "Type" | "Version" | "TryExec" | "Exec" | "Path" => Self::StringEntry,
            "Icon" => Self::IconEntry,

            // Lists
            "OnlyShowIn" | "NotShowIn" | "Actions" | "MimeType" | "Categories" | "Implements" => {
                Self::StringList
            }
            "Keywords" => Self::LocalizedStringList,

            // Special keys
            "X-Ubuntu-Gettext-Domain" => Self::StringEntry,
            _ => Self::Unknown,
        }
    }

    pub fn from_entry_value(value: &str) -> Self {
        if STRING_LIST_RE.is_match(value.trim()) {
            return Self::StringList;
        }

        match value {
            "true" | "false" => Self::BoolSwitch,
            _ => Self::Unknown,
        }
    }

    pub fn default_value(&self) -> &str {
        match self {
            EntryWidgetType::StringEntry
            | EntryWidgetType::LocalizedStringEntry
            | EntryWidgetType::IconEntry
            | EntryWidgetType::StringList
            | EntryWidgetType::LocalizedStringList
            | EntryWidgetType::Unknown => "",
            EntryWidgetType::BoolSwitch => "false",
        }
    }
}

/// Abstracts widgets that can change state, regardless of method
pub trait RowWidgetExt: PreferencesRowExt {
    fn connect_changed<F>(&self, f: F) -> SignalHandlerId
    where
        F: Fn(&Self) + 'static;

    fn entry_key(&self) -> String {
        self.title().to_string()
    }

    fn entry_value(&self) -> String;
    fn entry_locale(&self) -> Option<String>;
}

impl RowWidgetExt for StringEntryRow {
    fn connect_changed<F>(&self, f: F) -> SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        // Using `connect_text_notify` instead of `connect_changed` as it prevents duplicate
        // signals when programmatically changing the text (using `set_text`)
        self.connect_text_notify(f)
    }

    fn entry_value(&self) -> String {
        self.text().to_string()
    }

    fn entry_locale(&self) -> Option<String> {
        self.locale()
    }
}

impl RowWidgetExt for IconEntryRow {
    fn connect_changed<F>(&self, f: F) -> SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_activate(f)
    }

    fn entry_value(&self) -> String {
        self.text().to_string()
    }

    fn entry_locale(&self) -> Option<String> {
        None
    }
}

impl RowWidgetExt for SwitchRow {
    fn connect_changed<F>(&self, f: F) -> SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_active_notify(f)
    }

    fn entry_value(&self) -> String {
        match self.is_active() {
            true => "true".to_string(),
            false => "false".to_string(),
        }
    }

    fn entry_locale(&self) -> Option<String> {
        None
    }
}

impl RowWidgetExt for TaggedEntryRow {
    fn connect_changed<F>(&self, f: F) -> SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_closure("changed", true, closure_local!(|this: &Self| { f(this) }))
    }

    fn entry_value(&self) -> String {
        self.values_as_string_list()
    }

    fn entry_locale(&self) -> Option<String> {
        self.locale()
    }
}
