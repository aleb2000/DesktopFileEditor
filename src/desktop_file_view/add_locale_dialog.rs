use gtk::glib;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::{prelude::*, subclass::prelude::*};
    use gtk::{
        gdk::{Key, ModifierType},
        glib::{self, clone, closure, GString, Object, Propagation, Properties},
        Entry, EventControllerKey, Label,
    };

    use crate::desktop_file_view::{
        languages::LANGUAGES_LOCALE_MAP,
        util::{connect_self_fn, entry_popup_completion_handle_escape_key_pressed},
    };

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::AddLocaleDialog)]
    pub struct AddLocaleDialog {
        entry: RefCell<gtk::Entry>,
        language_label: RefCell<gtk::Label>,
        locale_exists_label: RefCell<gtk::Label>,

        #[property(get, set)]
        locale: RefCell<String>,

        #[property(get, set)]
        locale_exists: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AddLocaleDialog {
        const NAME: &'static str = "AddLocaleDialog";
        type Type = super::AddLocaleDialog;
        type ParentType = adw::AlertDialog;
    }

    #[glib::derived_properties]
    impl ObjectImpl for AddLocaleDialog {
        fn constructed(&self) {
            let obj = self.obj();

            obj.set_heading(Some("Add Locale"));
            obj.set_body("Choose a language to add");

            let container = gtk::Box::builder()
                .spacing(6)
                .orientation(gtk::Orientation::Vertical)
                .build();

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Locale"));

            // Connect entry signals
            entry.connect_changed(connect_self_fn!(self.on_entry_changed(entry)));
            entry.connect_activate(connect_self_fn!(self.on_entry_activated(entry)));
            let entry_controller_key = EventControllerKey::new();
            entry_controller_key.connect_key_pressed(connect_self_fn!(
                self.on_entry_key_pressed(controller, key, code, modifier) -> Propagation::Proceed
            ));
            entry.add_controller(entry_controller_key);

            let language_label = Label::builder()
                .halign(gtk::Align::Center)
                .justify(gtk::Justification::Center)
                .visible(false)
                .css_classes(["accent"])
                .build();

            let locale_exists_label = Label::builder()
                .halign(gtk::Align::Center)
                .justify(gtk::Justification::Center)
                .visible(false)
                .label("The locale already exists")
                .css_classes(["error"])
                .build();

            container.append(&entry);
            container.append(&language_label);
            container.append(&locale_exists_label);
            obj.set_extra_child(Some(&container));

            obj.add_responses(&[("cancel", "Cancel"), ("add", "Add")]);
            obj.set_response_appearance("add", adw::ResponseAppearance::Suggested);
            obj.set_response_enabled("add", false);

            entry
                .property_expression_weak("text")
                .chain_closure::<String>(closure!(|_: Option<Object>, s: &str| {
                    s.trim().to_string()
                }))
                .bind(&obj.clone(), "locale", Object::NONE);

            self.entry.replace(entry);
            self.language_label.replace(language_label);
            self.locale_exists_label.replace(locale_exists_label);

            self.init_completion();
            obj.connect_map(|dialog| {
                dialog.imp().entry.borrow().grab_focus();
            });
        }
    }

    impl AdwAlertDialogImpl for AddLocaleDialog {}
    impl AdwDialogImpl for AddLocaleDialog {}
    impl WidgetImpl for AddLocaleDialog {}

    impl AddLocaleDialog {
        #[allow(deprecated)]
        fn init_completion(&self) {
            let entry = self.entry.borrow().clone();
            let entry_model = gtk::ListStore::new(&[glib::Type::STRING, glib::Type::STRING]);
            for (locale, language) in LANGUAGES_LOCALE_MAP.iter() {
                entry_model.set(
                    &entry_model.append(),
                    &[(0, &format!("[{}] {}", locale, language)), (1, locale)],
                );
            }

            let completion = gtk::EntryCompletion::builder()
                .model(&entry_model)
                .text_column(0)
                .minimum_key_length(2)
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
                    let iter_text =
                        TreeModelExtManual::get::<GString>(&model, iter, 0).to_lowercase();
                    s.split_whitespace().all(|word| iter_text.contains(word))
                }
            ));

            completion.connect_match_selected(clone!(
                #[weak]
                entry,
                #[upgrade_or]
                Propagation::Proceed,
                move |_completion, model, iter| {
                    let locale = TreeModelExtManual::get::<GString>(model, iter, 1);
                    // Set text does not properly send the property modified signal, hence the use
                    // of delete and insert
                    entry.delete_text(0, entry.text().len() as i32);
                    entry.insert_text(&locale, &mut 0);
                    entry.select_region(0, locale.len() as i32);
                    Propagation::Stop
                }
            ));

            entry.set_completion(Some(&completion));
        }

        #[allow(deprecated)]
        fn on_entry_changed(&self, entry: &Entry) {
            // Make sure to enable completion on change
            entry.completion().unwrap().set_popup_completion(true);

            let text = entry.text();
            let locale = text.trim();

            let language_label = self.language_label.borrow();
            let locale_exists_label = self.locale_exists_label.borrow();

            entry.remove_css_class("accent");
            entry.remove_css_class("warning");
            entry.remove_css_class("error");
            let mut entry_css_class = match LANGUAGES_LOCALE_MAP.get(locale) {
                Some(language) => {
                    language_label.set_text(language);
                    language_label.set_css_classes(&["accent"]);
                    "accent"
                }
                None => {
                    language_label.set_text("Unknown language code");
                    language_label.set_css_classes(&["warning"]);
                    if locale.is_empty() {
                        ""
                    } else {
                        "warning"
                    }
                }
            };

            language_label.set_visible(!locale.is_empty());

            let locale_exists = self.locale_exists.get();
            locale_exists_label.set_visible(locale_exists);
            if locale_exists {
                entry_css_class = "error";
            }

            if !entry_css_class.is_empty() {
                entry.add_css_class(entry_css_class);
            }

            self.obj()
                .set_response_enabled("add", self.obj().valid_locale());
        }

        fn on_entry_activated(&self, _entry: &Entry) {
            let obj = self.obj();
            if obj.valid_locale() {
                obj.set_close_response("add");
                if !obj.close() {
                    eprintln!(
                        "Failed to close add locale dialog, closing forcefully, please report this bug!"
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
    pub struct AddLocaleDialog(ObjectSubclass<imp::AddLocaleDialog>)
        @extends adw::AlertDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl AddLocaleDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn valid_locale(&self) -> bool {
        !self.locale().is_empty() && !self.locale_exists()
    }
}

impl Default for AddLocaleDialog {
    fn default() -> Self {
        Self::new()
    }
}
