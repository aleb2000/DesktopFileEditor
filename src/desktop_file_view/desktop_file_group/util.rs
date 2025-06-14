use gtk::{gio::Menu, glib::variant::ToVariant};

use crate::desktop_file_view::known_entries::KEYS_DESCRIPTIONS;

pub fn make_additional_option_button(
    icon_name: &str,
    tooltip_text: &str,
    action_name: &str,
    action_target: &str,
    extra_css_class: &str,
) -> gtk::Widget {
    gtk::Button::builder()
        .icon_name(icon_name)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .tooltip_text(tooltip_text)
        .action_name(action_name)
        .action_target(&action_target.to_variant())
        .css_classes(["circular", extra_css_class])
        .build()
        .into()
}

pub fn make_additional_options_menu(key: &str) -> gtk::Widget {
    let remove_only = !KEYS_DESCRIPTIONS.contains_key(key);

    if remove_only {
        return make_additional_option_button(
            "list-remove-symbolic",
            "Remove",
            "desktop_file_group.remove_entry",
            key,
            "destructive-action",
        );
    }

    let menu = Menu::new();
    menu.append(
        Some("Remove"),
        Some(&format!("desktop_file_group.remove_entry('{}')", key)),
    );

    if !remove_only {
        menu.append(
            Some("Description"),
            Some(&format!("desktop_file_group.show_entry_info('{}')", key)),
        );
    }

    gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .menu_model(&menu)
        .css_classes(["circular"])
        .tooltip_text("More options")
        .build()
        .into()
}

pub fn add_additional_options_buttons<F>(key: &str, add_fn: F)
where
    F: Fn(&gtk::Widget),
{
    let remove_only = !KEYS_DESCRIPTIONS.contains_key(key);

    add_fn(&make_additional_option_button(
        "list-remove-symbolic",
        "Remove",
        "desktop_file_group.remove_entry",
        key,
        "destructive-action",
    ));

    if !remove_only {
        add_fn(&make_additional_option_button(
            "help-about-symbolic",
            "Description",
            "desktop_file_group.show_entry_info",
            key,
            "",
        ));
    }
}
