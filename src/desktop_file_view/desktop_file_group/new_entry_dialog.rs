use std::borrow::Borrow;

use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    glib::{self, clone::Downgrade},
    prelude::EditableExt,
};

use crate::desktop_file_view::{
    desktop_entry_ext::DesktopEntryExt, imp::DesktopEntryCell, DesktopFileView,
};
use once_cell::sync::Lazy;
use regex::Regex;

pub static VALID_KEY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[A-Za-z0-9-]*$").expect("Failed to compile regex"));

mod imp {

    use std::cell::RefCell;

    use adw::{prelude::*, subclass::prelude::*};
    use gtk::glib::clone::Downgrade;
    use gtk::{
        gdk::{Key, ModifierType},
        glib::{self, clone, GString, Propagation, Properties},
        Entry, EventControllerKey, Image, Label, ScrolledWindow,
    };

    use crate::desktop_file_view::{
        known_entries::{KEYS_DESCRIPTIONS, KNOWN_KEYS},
        util::{connect_self_fn, entry_popup_completion_handle_escape_key_pressed},
        DesktopFileView,
    };

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::NewEntryDialog)]
    pub struct NewEntryDialog {
        pub entry: RefCell<gtk::Entry>,
        fail_label: RefCell<gtk::Label>,
        info_label: RefCell<gtk::Label>,
        info_box: RefCell<gtk::Box>,

        pub desktop_file_view: RefCell<Option<<DesktopFileView as Downgrade>::Weak>>,

        #[property(get, set, construct)]
        pub group_name: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NewEntryDialog {
        const NAME: &'static str = "NewEntryDialog";
        type Type = super::NewEntryDialog;
        type ParentType = adw::AlertDialog;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NewEntryDialog {
        fn constructed(&self) {
            let obj = self.obj();

            obj.set_heading(Some("Add New Entry"));

            let container = gtk::Box::builder()
                .spacing(6)
                .orientation(gtk::Orientation::Vertical)
                .build();

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Entry key"));

            // Connect entry signals
            entry.connect_changed(connect_self_fn!(self.on_entry_changed(entry)));
            entry.connect_activate(connect_self_fn!(self.on_entry_activated(entry)));
            let entry_controller_key = EventControllerKey::new();
            entry_controller_key.connect_key_pressed(connect_self_fn!(
                self.on_entry_key_pressed(controller, key, code, modifier) -> Propagation::Proceed
            ));
            entry.add_controller(entry_controller_key);

            let fail_label = Label::builder()
                .halign(gtk::Align::Center)
                .justify(gtk::Justification::Center)
                .visible(false)
                .wrap(true)
                .css_classes(["error"])
                .build();

            let info_box = gtk::Box::builder()
                .halign(gtk::Align::Center)
                .spacing(12)
                .visible(false)
                .css_classes(["accent"])
                .build();

            let info_icon = Image::builder()
                .icon_name("help-about-symbolic")
                .build();

            let info_label = Label::builder().wrap(true).build();

            let info_scroll = ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .max_content_height(200)
                .propagate_natural_height(true)
                .propagate_natural_width(true)
                .child(&info_label)
                .build();

            info_box.append(&info_icon);
            info_box.append(&info_scroll);

            container.append(&entry);
            container.append(&fail_label);
            container.append(&info_box);
            obj.set_extra_child(Some(&container));

            obj.add_responses(&[("cancel", "Cancel"), ("add", "Add")]);
            obj.set_response_appearance("add", adw::ResponseAppearance::Suggested);
            obj.set_response_enabled("add", false);

            self.entry.replace(entry);
            self.fail_label.replace(fail_label);
            self.info_label.replace(info_label);
            self.info_box.replace(info_box);

            self.init_completion();
            obj.connect_map(|dialog| {
                dialog.imp().entry.borrow().grab_focus();
            });
        }
    }

    impl AdwAlertDialogImpl for NewEntryDialog {}
    impl AdwDialogImpl for NewEntryDialog {}
    impl WidgetImpl for NewEntryDialog {}

    impl NewEntryDialog {
        #[allow(deprecated)]
        fn init_completion(&self) {
            // Setup key entry completion
            let entry = self.entry.borrow();
            let entry_model = gtk::ListStore::new(&[glib::Type::STRING]);
            for key in KNOWN_KEYS {
                entry_model.set(&entry_model.append(), &[(0, &key)]);
            }

            let completion = gtk::EntryCompletion::builder()
                .model(&entry_model)
                .text_column(0)
                .minimum_key_length(0)
                .popup_completion(false)
                .build();

            let completion_cell = gtk::CellRendererText::new();
            completion_cell.set_xpad(6);
            completion.pack_start(&completion_cell, false);
            completion.add_attribute(&completion_cell, "text", 0);
            completion.set_match_func(clone!(
                #[weak(rename_to=model)]
                entry_model,
                #[upgrade_or]
                false,
                move |_completion, s, iter| {
                    let iter_text = TreeModelExtManual::get::<GString>(&model, iter, 0);
                    iter_text.to_lowercase().contains(s)
                }
            ));

            entry.set_completion(Some(&completion));
        }

        #[allow(deprecated)]
        fn on_entry_changed(&self, entry: &Entry) {
            let obj = self.obj();

            // Make sure to enable completion on change
            entry.completion().unwrap().set_popup_completion(true);

            let key = obj.validated_key();

            let fail_label = self.fail_label.borrow();
            let info_label = self.info_label.borrow();
            let info_box = self.info_box.borrow();

            let fail_reason = key.as_ref().map_or_else(
                |err| {
                    err.fail_messages()
                        .into_iter()
                        .map(|s| "• ".to_string() + s)
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                |_| String::new(),
            );
            fail_label.set_text(&fail_reason);

            if key.is_err() {
                entry.add_css_class("error");
                fail_label.set_visible(true);
            } else {
                entry.remove_css_class("error");
                fail_label.set_visible(false);
            }

            // Update info
            if let Some(description) =
                KEYS_DESCRIPTIONS.get(key.as_ref().map(|s| &s[..]).unwrap_or(""))
            {
                info_label.set_text(description);
                info_box.set_visible(true);
            } else {
                info_label.set_text("");
                info_box.set_visible(false);
            }

            self.obj().set_response_enabled("add", key.is_ok());
        }

        fn on_entry_activated(&self, _entry: &Entry) {
            let obj = self.obj();
            if obj.validated_key().is_ok() {
                obj.set_close_response("add");
                if !obj.close() {
                    eprintln!(
                        "Failed to close new entry dialog, closing forcefully, please report this bug!"
                    );
                    obj.force_close();
                }
                obj.set_close_response("cancel");
            }
        }

        fn on_entry_key_pressed(
            &self,
            _controller: &gtk::EventControllerKey,
            key: Key,
            _code: u32,
            modifier: ModifierType,
        ) -> Propagation {
            entry_popup_completion_handle_escape_key_pressed(&self.entry.borrow(), key, modifier)
                .into()
        }
    }
}

glib::wrapper! {
    pub struct NewEntryDialog(ObjectSubclass<imp::NewEntryDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl NewEntryDialog {
    pub fn new(
        group_name: String,
        desktop_file_view: <DesktopFileView as Downgrade>::Weak,
    ) -> Self {
        let obj: NewEntryDialog = glib::Object::builder()
            .property("group-name", group_name)
            .build();
        let imp = obj.imp();
        imp.desktop_file_view.replace(Some(desktop_file_view));
        obj
    }

    pub fn validated_key(&self) -> Result<String, KeyValidationError> {
        let imp = self.imp();
        let key = imp.entry.borrow().text().trim().to_string();

        let desktop_file_view = imp.desktop_file_view.borrow().as_ref().unwrap().upgrade();
        let key_already_exists = if let Some(desktop_file_view) = desktop_file_view {
            let desktop_entry_rc = &desktop_file_view.desktop_entry();
            let desktop_entry_cell: &DesktopEntryCell = std::rc::Rc::borrow(desktop_entry_rc);
            let desktop_entry = desktop_entry_cell.borrow();

            if let Some(keymap) = desktop_entry.sorted_keymap(&self.group_name()) {
                keymap
                    .iter()
                    .map(|(existing_key, _)| existing_key)
                    .any(|existing_key| existing_key == &key)
            } else {
                false
            }
        } else {
            false
        };

        let valid_key = VALID_KEY_RE.is_match(&key);
        if !key.is_empty() && !key_already_exists && valid_key {
            Ok(key)
        } else {
            Err(KeyValidationError {
                empty_key: key.is_empty(),
                key_exists: key_already_exists,
                invalid_key: !valid_key,
            })
        }
    }
}

#[derive(Debug)]
pub struct KeyValidationError {
    empty_key: bool,
    key_exists: bool,
    invalid_key: bool,
}

impl KeyValidationError {
    pub fn fail_messages(&self) -> Vec<&str> {
        let mut v = Vec::new();
        if self.empty_key {
            v.push("The key is empty");
        }
        if self.key_exists {
            v.push("A key with the same name already exists");
        }
        if self.invalid_key {
            v.push("Keys can only contain alphanumerical characters (A-Z, a-z, 0-9) and the hypen symbol (-)");
        }
        v
    }
}
