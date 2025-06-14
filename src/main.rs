use std::env;

use adw::prelude::*;
use application::DMApplication;
use gtk::gio;
use gtk::glib;

mod application;
mod desktop_file_view;
mod window;

const APP_ID: &str = "org.argoware.desktop_manager";

fn main() -> glib::ExitCode {
    gio::resources_register_include!("desktop_manager.gresource")
        .expect("Failed to register resources");

    let app = DMApplication::default();

    app.run()
}
