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
    use std::cell::Cell;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib::{self, Properties};
    use gtk::glib::{
        object_subclass,
        subclass::{object::ObjectImpl, types::ObjectSubclass},
    };
    use gtk::subclass::filter::FilterImpl;

    use crate::window::file_entry::{FileEntry, ShouldShow};

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::EntryFilter)]
    pub struct EntryFilter {
        #[property(get, set)]
        pub only_show_selected: Cell<bool>,

        #[property(get, set)]
        pub hidden: Cell<bool>,

        #[property(get, set)]
        pub invalid: Cell<bool>,
    }

    #[object_subclass]
    impl ObjectSubclass for EntryFilter {
        const NAME: &'static str = "EntryFilter";
        type Type = super::EntryFilter;
        type ParentType = gtk::Filter;
    }

    #[glib::derived_properties]
    impl ObjectImpl for EntryFilter {
        fn constructed(&self) {
            // Make sure filter is updated on property change
            let obj = self.obj();
            obj.connect_only_show_selected_notify(filter_updated);
            obj.connect_hidden_notify(filter_updated);
            obj.connect_invalid_notify(filter_updated);
        }
    }

    impl FilterImpl for EntryFilter {
        fn match_(&self, item: &glib::Object) -> bool {
            let file_entry = item
                .clone()
                .downcast::<FileEntry>()
                .expect("item should be `EntryObj`");

            if self.only_show_selected.get() {
                let mut matches = false;

                if self.hidden.get() {
                    matches |= !matches!(file_entry.should_show(), ShouldShow::Yes);
                }

                if self.invalid.get() {
                    matches |= !file_entry.validity_status().is_valid();
                }

                matches
            } else {
                let mut matches = true;

                if !self.hidden.get() {
                    matches &= matches!(file_entry.should_show(), ShouldShow::Yes);
                }

                if !self.invalid.get() {
                    matches &= file_entry.validity_status().is_valid();
                }

                matches
            }
        }
    }

    fn filter_updated(filter: &super::EntryFilter) {
        filter.changed(gtk::FilterChange::Different);
    }
}

glib::wrapper! {
    pub struct EntryFilter(ObjectSubclass<imp::EntryFilter>)
    @extends gtk::Filter;
}

impl EntryFilter {
    pub fn new(only_show_selected: bool, show_hidden: bool, show_invalid: bool) -> Self {
        glib::Object::builder()
            .property("only-show-selected", only_show_selected)
            .property("hidden", show_hidden)
            .property("invalid", show_invalid)
            .build()
    }
}

impl Default for EntryFilter {
    fn default() -> Self {
        Self::new(false, false, false)
    }
}
