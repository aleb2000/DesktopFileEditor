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

use adw::glib;
use gtk::{glib::Object, subclass::prelude::ObjectSubclassIsExt, Image, Label};

mod imp {

    use std::cell::Cell;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::process::Command;

    use adw::glib;
    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::gdk::Rectangle;

    use gtk::glib::closure;
    use gtk::glib::object_subclass;
    use gtk::glib::subclass::InitializingObject;
    use gtk::glib::Object;
    use gtk::glib::Properties;
    use gtk::glib::Variant;

    use gtk::template_callbacks;
    use gtk::Image;
    use gtk::Label;
    use gtk::PopoverMenu;
    use gtk::Widget;
    use gtk::{subclass::prelude::ObjectSubclass, CompositeTemplate};
    use zbus::proxy;
    use zbus::Connection;

    use crate::util::display_path;
    use crate::window::file_entry::ShouldShow;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::ListEntry)]
    #[template(resource = "/com/argoware/desktop-file-editor/list_entry.ui")]
    pub struct ListEntry {
        #[template_child]
        pub name_label: TemplateChild<Label>,

        #[template_child]
        pub path_label: TemplateChild<Label>,

        #[template_child]
        pub icon_image: TemplateChild<Image>,

        #[template_child]
        pub context_menu: TemplateChild<PopoverMenu>,

        #[template_child]
        pub hidden_marker: TemplateChild<Image>,

        #[template_child]
        pub invalid_marker: TemplateChild<Image>,

        #[property(get, set, builder(ShouldShow::default()))]
        pub should_show: Cell<ShouldShow>,

        #[property(get, set)]
        pub path: RefCell<PathBuf>,
    }

    #[object_subclass]
    impl ObjectSubclass for ListEntry {
        const NAME: &'static str = "DMListEntry";
        type Type = super::ListEntry;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action_async(
                "list_entry.open_location",
                None,
                open_item_location_handler,
            );
            klass.install_action("list_entry.open", None, |list_entry, _, _| {
                let path = list_entry.path();
                let _ = Command::new("xdg-open").arg(path).spawn().unwrap().wait();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ListEntry {
        fn constructed(&self) {
            let obj = self.obj();

            obj.property_expression("path")
                .chain_closure::<PathBuf>(closure!(|_: Option<Object>, path: PathBuf| {
                    display_path(&path)
                }))
                .bind(&obj.path_label(), "label", Widget::NONE);
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }
    impl WidgetImpl for ListEntry {}
    impl BoxImpl for ListEntry {}

    #[template_callbacks]
    impl ListEntry {
        #[template_callback]
        fn on_gestureclick_released(&self, _n_press: i32, x: f64, y: f64) {
            self.context_menu
                .set_pointing_to(Some(&Rectangle::new(x as i32, y as i32, 1, 1)));
            self.context_menu.popup();
        }

        #[template_callback]
        fn is_hidden(&self, should_show: ShouldShow) -> bool {
            !matches!(should_show, ShouldShow::Yes)
        }

        #[template_callback]
        fn hidden_marker_tooltip_markup(&self, should_show: ShouldShow) -> &str {
            match should_show {
                ShouldShow::Yes => "",
                ShouldShow::NoDisplay => "<b>NoDisplay</b>",
                ShouldShow::Hidden => "<b>Hidden</b>",
                ShouldShow::NoDisplayAndHidden => "<b>NoDisplay</b> and <b>Hidden</b>",
            }
        }
    }

    #[proxy(
        interface = "org.freedesktop.FileManager1",
        default_service = "org.freedesktop.FileManager1",
        default_path = "/org/freedesktop/FileManager1"
    )]
    trait FileManagerInterface {
        fn show_items(&self, paths: &[&str], startup_id: &str) -> zbus::Result<()>;
    }

    async fn open_item_location_handler(
        list_entry: super::ListEntry,
        _: String,
        _: Option<Variant>,
    ) {
        let path = format!("file://{}", list_entry.path().to_string_lossy());
        let connection = Connection::session().await.unwrap();
        let proxy = FileManagerInterfaceProxy::new(&connection).await.unwrap();
        proxy.show_items(&[&path], "").await.unwrap();
    }
}

glib::wrapper! {
    pub struct ListEntry(ObjectSubclass<imp::ListEntry>)
    @extends gtk::Widget, gtk::Box,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ListEntry {
    pub fn new() -> Self {
        Object::builder().build()
    }

    pub fn name_label(&self) -> Label {
        self.imp().name_label.clone()
    }

    fn path_label(&self) -> Label {
        self.imp().path_label.clone()
    }

    pub fn icon_image(&self) -> Image {
        self.imp().icon_image.clone()
    }

    pub fn hidden_marker(&self) -> Image {
        self.imp().hidden_marker.clone()
    }

    pub fn invalid_marker(&self) -> Image {
        self.imp().invalid_marker.clone()
    }
}

impl Default for ListEntry {
    fn default() -> Self {
        Self::new()
    }
}
