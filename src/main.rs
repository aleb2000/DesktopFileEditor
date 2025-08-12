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

use std::env;

use adw::prelude::*;
use application::DMApplication;
use gtk::gio;
use gtk::glib;

mod application;
mod desktop_file_view;
mod window;
mod shellparse;

const APP_ID: &str = "com.argoware.desktop-file-editor";

fn main() -> glib::ExitCode {
    gio::resources_register_include!("desktop_file_editor.gresource")
        .expect("Failed to register resources");

    let app = DMApplication::default();

    app.run()
}
