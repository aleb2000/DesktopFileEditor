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

use gtk::{
    gdk::{Key, ModifierType},
    glib::{self, clone::Downgrade, closure_local, subclass::types::ObjectSubclassIsExt},
    EventControllerKey, SearchEntry, Widget,
};

use gtk::prelude::*;

mod imp {
    use gtk::prelude::*;
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::{
        glib::{
            self,
            subclass::{object::ObjectImpl, types::ObjectSubclass, InitializingObject},
        },
        subclass::widget::{CompositeTemplateClass, WidgetImpl},
    };
    use gtk::{
        Button, CompositeTemplate, EventControllerFocus, EventControllerKey, Revealer, SearchEntry,
        Widget,
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/argoware/desktop-file-editor/sliding_search_entry.ui")]
    pub struct SlidingSearchEntry {
        #[template_child]
        pub revealer: TemplateChild<Revealer>,

        #[template_child]
        pub search_entry: TemplateChild<SearchEntry>,

        #[template_child]
        pub button: TemplateChild<Button>,

        pub capture_widget: RefCell<Option<Widget>>,
        pub capture_widget_controller: RefCell<Option<EventControllerKey>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SlidingSearchEntry {
        const NAME: &'static str = "SlidingSearchEntry";
        type Type = super::SlidingSearchEntry;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SlidingSearchEntry {
        fn constructed(&self) {
            let search_entry = self.search_entry.clone();
            let button = self.button.clone();
            self.revealer
                .connect_child_revealed_notify(move |revealer| {
                    if revealer.reveals_child() {
                        button.add_css_class("sliding-search-entry-revealed");
                        search_entry.grab_focus();
                    }
                });

            self.search_entry
                .set_key_capture_widget(Some(&self.obj().clone()));
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }
    impl WidgetImpl for SlidingSearchEntry {}

    #[gtk::template_callbacks]
    impl SlidingSearchEntry {
        #[template_callback]
        fn on_button_clicked(&self, _button: &Button) {
            let will_reveal = !self.revealer.is_child_revealed();
            self.set_reveal(will_reveal);
        }

        #[template_callback]
        fn on_eventcontrollerfocus_leave(&self, _event_controller_focus: &EventControllerFocus) {
            if self.search_entry.text().is_empty() {
                self.set_reveal(false);
            }
        }

        #[template_callback]
        fn on_eventcontrollerfocus_enter(&self, _event_controller_focus: &EventControllerFocus) {
            if !self.revealer.reveals_child() {
                self.set_reveal(true);
            }
        }

        #[template_callback]
        fn on_searchentry_stop_search(&self, _search_entry: &SearchEntry) {
            self.set_reveal(false);
        }

        #[template_callback]
        fn on_searchentry_changed(&self, search_entry: &SearchEntry) {
            if !search_entry.text().is_empty() && !self.revealer.reveals_child() {
                self.set_reveal(true);
            }
        }

        pub fn set_reveal(&self, reveal: bool) {
            self.revealer.set_reveal_child(reveal);
            if !reveal {
                self.button
                    .remove_css_class("sliding-search-entry-revealed");
                self.search_entry.set_text("");
            }
        }
    }
}

glib::wrapper! {
    pub struct SlidingSearchEntry(ObjectSubclass<imp::SlidingSearchEntry>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SlidingSearchEntry {
    pub fn search_entry(&self) -> SearchEntry {
        self.imp().search_entry.clone()
    }

    pub fn set_reveal(&self, reveal: bool) {
        self.imp().set_reveal(reveal);
    }

    pub fn reveals_child(&self) -> bool {
        self.imp().revealer.reveals_child()
    }

    // The key capture widget implementation is heavily inspired by the same feature in the
    // SearchBar widget, although adapted and translated to Rust:
    // https://gitlab.gnome.org/GNOME/gtk/-/blob/main/gtk/gtksearchbar.c
    pub fn set_key_capture_widget(&self, widget: Option<Widget>) {
        let imp = self.imp();

        if *imp.capture_widget.borrow() == widget {
            return;
        }

        if let Some(capture_widget) = imp.capture_widget.borrow().clone() {
            let controller = imp
                .capture_widget_controller
                .borrow()
                .clone()
                .expect("A capture widget is set but no controller");
            capture_widget.remove_controller(&controller);
            *imp.capture_widget_controller.borrow_mut() = None;
        }

        imp.capture_widget.borrow_mut().clone_from(&widget);

        if let Some(widget) = widget {
            let capture_widget_controller = EventControllerKey::builder()
                .propagation_phase(gtk::PropagationPhase::Bubble)
                .build();

            let sliding_search_entry = Downgrade::downgrade(self);
            capture_widget_controller.connect_closure(
                "key-pressed",
                true,
                closure_local!(|controller: &EventControllerKey,
                                keyval: Key,
                                keycode: u32,
                                state: ModifierType| {
                    let sliding_search_entry = glib::clone::Upgrade::upgrade(&sliding_search_entry)
                        .expect("Unable to upgrade weak reference");
                    Self::capture_widget_key_handled(
                        controller,
                        keyval,
                        keycode,
                        state,
                        &sliding_search_entry,
                    )
                }),
            );

            let sliding_search_entry = Downgrade::downgrade(self);
            capture_widget_controller.connect_closure(
                "key-released",
                true,
                closure_local!(|controller: &EventControllerKey,
                                keyval: Key,
                                keycode: u32,
                                state: ModifierType| {
                    let sliding_search_entry = glib::clone::Upgrade::upgrade(&sliding_search_entry)
                        .expect("Unable to upgrade weak reference");
                    Self::capture_widget_key_handled(
                        controller,
                        keyval,
                        keycode,
                        state,
                        &sliding_search_entry,
                    );
                }),
            );

            widget.add_controller(capture_widget_controller.clone());

            *imp.capture_widget_controller.borrow_mut() = Some(capture_widget_controller);
        }
    }

    fn capture_widget_key_handled(
        controller: &EventControllerKey,
        _: Key,
        _: u32,
        _: ModifierType,
        sliding_search_entry: &SlidingSearchEntry,
    ) -> bool {
        if !sliding_search_entry.is_mapped() {
            return false;
        }

        let handled = controller.forward(sliding_search_entry);
        if handled {
            sliding_search_entry.set_reveal(true);
        }
        handled
    }
}
