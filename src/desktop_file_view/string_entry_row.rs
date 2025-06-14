use adw::prelude::*;
use gtk::glib;
use gtk::glib::property::PropertySet;
use gtk::glib::subclass::types::ObjectSubclassIsExt;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::Separator;
    use gtk::{
        glib::{self, closure, Object, Properties},
        Label, Widget,
    };

    use crate::desktop_file_view::languages::LANGUAGES_LOCALE_MAP;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::StringEntryRow)]
    pub struct StringEntryRow {
        #[property(get, set)]
        pub localizable: Cell<bool>,

        #[property(get, set)]
        pub locale: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StringEntryRow {
        const NAME: &'static str = "StringEntryRow";
        type Type = super::StringEntryRow;
        type ParentType = adw::EntryRow;
    }

    #[glib::derived_properties()]
    impl ObjectImpl for StringEntryRow {
        fn constructed(&self) {
            let obj = self.obj();

            let spacer = Separator::builder()
                .orientation(gtk::Orientation::Horizontal)
                .margin_start(6)
                .build();
            obj.add_prefix(&spacer);

            let locale_label = Label::builder().tooltip_text("Locale").build();

            obj.add_prefix(&locale_label);

            let prefixes = obj
                .first_child()
                .expect("Could not get entry object header")
                .first_child()
                .expect("Could not get entry object prefixes")
                .downcast::<gtk::Box>()
                .expect("Prefixes is not a Box");

            obj.property_expression_weak("locale")
                .chain_closure::<bool>(closure!(|_: Option<Object>, locale: Option<String>| {
                    locale.is_some()
                }))
                .bind(&prefixes, "visible", Widget::NONE);

            obj.property_expression_weak("locale")
                .bind(&locale_label, "label", Widget::NONE);

            obj.property_expression_weak("locale")
                .chain_closure::<String>(closure!(|_: Option<Object>, locale: Option<String>| {
                    LANGUAGES_LOCALE_MAP
                        .get(&locale.unwrap_or_default()[..])
                        .unwrap_or(&"Locale")
                        .to_string()
                }))
                .bind(&locale_label, "tooltip_text", Widget::NONE);
        }
    }

    impl EntryRowImpl for StringEntryRow {}
    impl PreferencesRowImpl for StringEntryRow {}
    impl ListBoxRowImpl for StringEntryRow {}
    impl WidgetImpl for StringEntryRow {}
}

glib::wrapper! {
    pub struct StringEntryRow(ObjectSubclass<imp::StringEntryRow>)
        @extends adw::EntryRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget, gtk::Editable;
}

impl StringEntryRow {
    pub fn new(localizable: bool, locale: Option<String>) -> Self {
        glib::Object::builder()
            .property("localizable", localizable)
            .property("locale", locale)
            .build()
    }

    pub fn with_default_locale(localizable: bool) -> Self {
        Self::new(localizable, None)
    }

    pub fn change_locale(&self, locale: Option<String>, localized_text: &str) {
        if !self.localizable() {
            return;
        }

        let freeze_guard = self.freeze_notify();
        let imp = self.imp();
        imp.locale.set(locale);
        self.notify_locale();

        self.set_text(localized_text);
        drop(freeze_guard);
    }
}
