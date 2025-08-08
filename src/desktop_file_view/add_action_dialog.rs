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

use gtk::glib;

mod imp {

    use std::cell::RefCell;

    use adw::{prelude::*, subclass::prelude::*};
    use gtk::glib::{closure, Object};
    use gtk::{
        glib::{self, clone, Properties},
        Entry,
    };

    use crate::desktop_file_view::util::connect_self_fn;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::AddActionDialog)]
    pub struct AddActionDialog {
        pub entry: RefCell<gtk::Entry>,

        #[property(get, set)]
        action: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AddActionDialog {
        const NAME: &'static str = "AddActionDialog";
        type Type = super::AddActionDialog;
        type ParentType = adw::AlertDialog;
    }

    #[glib::derived_properties]
    impl ObjectImpl for AddActionDialog {
        fn constructed(&self) {
            let obj = self.obj();

            obj.set_heading(Some("Add Action"));
            obj.set_body("An action represents an additional way to invoke the application");

            let container = gtk::Box::builder()
                .spacing(6)
                .orientation(gtk::Orientation::Vertical)
                .build();

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Action Identifier"));

            // Connect entry signals
            entry.connect_changed(connect_self_fn!(self.on_entry_changed(entry)));
            entry.connect_activate(connect_self_fn!(self.on_entry_activated(entry)));

            container.append(&entry);
            obj.set_extra_child(Some(&container));

            obj.add_responses(&[("cancel", "Cancel"), ("add", "Add")]);
            obj.set_response_appearance("add", adw::ResponseAppearance::Suggested);
            obj.set_response_enabled("add", false);

            entry
                .property_expression_weak("text")
                .chain_closure::<String>(closure!(|_: Option<Object>, s: &str| {
                    s.trim().to_string()
                }))
                .bind(&obj.clone(), "action", Object::NONE);

            self.entry.replace(entry);

            obj.connect_map(|dialog| {
                dialog.imp().entry.borrow().grab_focus();
            });
        }
    }

    impl AdwAlertDialogImpl for AddActionDialog {}
    impl AdwDialogImpl for AddActionDialog {}
    impl WidgetImpl for AddActionDialog {}

    impl AddActionDialog {
        fn on_entry_changed(&self, _entry: &Entry) {
            self.obj()
                .set_response_enabled("add", !self.obj().action().is_empty());
        }

        fn on_entry_activated(&self, _entry: &Entry) {
            let obj = self.obj();
            if !self.obj().action().is_empty() {
                obj.set_close_response("add");
                if !obj.close() {
                    eprintln!(
                        "Failed to close add action dialog, closing forcefully, please report this bug!"
                    );
                    obj.force_close();
                }
                obj.set_close_response("cancel");
            }
        }
    }
}

glib::wrapper! {
    pub struct AddActionDialog(ObjectSubclass<imp::AddActionDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl AddActionDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}

impl Default for AddActionDialog {
    fn default() -> Self {
        Self::new()
    }
}
