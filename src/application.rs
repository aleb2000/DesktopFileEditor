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
    gio::{self, prelude::*},
    glib::{self, OptionFlags},
};

use crate::APP_ID;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib::property::PropertySet;
    use gtk::glib;
    use gtk::subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclassExt,
    };
    use gtk::{glib::object_subclass, subclass::prelude::ObjectSubclass};

    use crate::window::DMWindow;

    #[derive(Debug, Default)]
    pub struct DMApplication {
        additional_search_paths: RefCell<Vec<String>>,
        ignore_default_paths: Cell<bool>,
    }

    #[object_subclass]
    impl ObjectSubclass for DMApplication {
        const NAME: &'static str = "DMApplication";
        type Type = super::DMApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for DMApplication {}

    impl ApplicationImpl for DMApplication {
        fn activate(&self) {
            self.parent_activate();
            let additional_search_paths = self.additional_search_paths.replace(Vec::new());
            let ignore_default_paths = self.ignore_default_paths.get();
            let window = DMWindow::new(&self.obj(), additional_search_paths, ignore_default_paths);
            window.present();
        }

        fn handle_local_options(&self, options: &glib::VariantDict) -> glib::ExitCode {
            let additional_search_paths = options
                .lookup::<Vec<String>>("add-search-path")
                .expect("Failed to lookup option")
                .unwrap_or_default();

            self.additional_search_paths.set(additional_search_paths);

            let ignore_default_paths = options.lookup::<bool>("ignore-default-paths")
                .expect("Failed to lookup option")
                .unwrap_or(false);

            self.ignore_default_paths.set(ignore_default_paths);

            self.parent_handle_local_options(options)
        }
    }
    impl GtkApplicationImpl for DMApplication {}
    impl AdwApplicationImpl for DMApplication {}
}

glib::wrapper! {
    pub struct DMApplication(ObjectSubclass<imp::DMApplication>)
        @extends gio::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for DMApplication {
    fn default() -> Self {
        let app: Self = glib::Object::builder()
            .property("application-id", APP_ID)
            .build();

        app.add_main_option("add-search-path", b'a'.into(), OptionFlags::NONE, glib::OptionArg::StringArray, "Add a path to look for desktop files in, besides the default ones. Can be used multiple times.", None);
        app.add_main_option("ignore-default-paths", b'i'.into(), OptionFlags::NONE, glib::OptionArg::None, "Don't look for desktop files in the default paths", None);
        app
    }
}
