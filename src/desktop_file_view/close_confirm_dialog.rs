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
