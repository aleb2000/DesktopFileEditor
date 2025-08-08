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

use adw::prelude::*;
use gtk::gio::Cancellable;

pub fn show_close_confirm_dialog<F, U>(parent: &impl IsA<gtk::Widget>, cancel: F, discard: U)
where
    F: Fn() + 'static,
    U: Fn() + 'static,
{
    let dialog = adw::AlertDialog::builder()
        .heading("Close File?")
        .body("Unsaved changes will be discarded")
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("discard", "Discard");
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);

    dialog.choose(parent, None::<&Cancellable>, move |response| {
        if response == "discard" {
            discard();
        } else {
            cancel();
        }
    });
}
