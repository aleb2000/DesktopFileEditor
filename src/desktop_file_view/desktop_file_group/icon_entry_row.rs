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
use gtk::glib;
use gtk::glib::subclass::types::ObjectSubclassIsExt;

use crate::window::file_entry::ToGIcon;

mod imp {
    use std::{cell::RefCell, rc::Rc};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::{
        gio::Cancellable,
        glib::{self, clone, closure_local, object::Cast, property::PropertySet},
        Align, ClosureExpression, Expression, FileDialog, FileFilter, FilterListModel, GridView,
        IconTheme, Image, ListItem, NoSelection, Orientation, Revealer,
        RevealerTransitionType, ScrolledWindow, SearchEntry, SignalListItemFactory, StringFilter,
        StringFilterMatchMode, StringList, StringObject,
    };

    const POPOVER_SIZE_SMALL: f64 = 85.0;
    const POPOVER_SIZE_LARGE: f64 = 360.0;
    const POPOVER_ANIM_DURATION: u32 = 325;

    #[derive(Default)]
    pub struct IconEntryRow {
        pub icon_image: RefCell<gtk::Image>,
        pub edit_button: RefCell<gtk::Button>,
        icon_search_filter: Rc<RefCell<StringFilter>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IconEntryRow {
        const NAME: &'static str = "IconEntryRow";
        type Type = super::IconEntryRow;
        type ParentType = adw::EntryRow;
    }

    impl ObjectImpl for IconEntryRow {
        fn constructed(&self) {
            let obj = self.obj();
            let entry_row: adw::EntryRow = obj.clone().upcast();
            entry_row.set_show_apply_button(true);

            // Prefix
            let icon_image = gtk::Image::builder()
                .pixel_size(32)
                .css_classes(["lowres-icon"])
                .build();
            entry_row.add_prefix(&icon_image);
            self.icon_image.set(icon_image);

            // Suffix
            let popover = gtk::Popover::new();
            popover.set_child(Some(&self.make_icon_chooser(&popover)));

            let menu_button = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .halign(Align::Center)
                .valign(Align::Center)
                .popover(&popover)
                .css_classes(["circular"])
                .build();
            entry_row.add_suffix(&menu_button);

            // Redirect apply to activate
            obj.connect_apply(|icon_entry_row| {
                icon_entry_row.activate();
            });

            obj.connect_activate(|icon_entry_row| {
                icon_entry_row.update_icon();
            });
        }
    }

    impl EntryRowImpl for IconEntryRow {}
    impl PreferencesRowImpl for IconEntryRow {}
    impl ListBoxRowImpl for IconEntryRow {}
    impl WidgetImpl for IconEntryRow {}

    impl IconEntryRow {
        fn make_icon_chooser(&self, popover: &gtk::Popover) -> gtk::Widget {
            popover.set_size_request(POPOVER_SIZE_SMALL as i32, POPOVER_SIZE_SMALL as i32);

            // Main options
            let file_button = gtk::Button::builder()
                .child(
                    &gtk::Label::builder()
                        .label("File")
                        .halign(Align::Start)
                        .build(),
                )
                .css_classes(["flat", "icon_chooser_menu_button"])
                .build();

            let icon_button = gtk::Button::builder()
                .child(
                    &gtk::Label::builder()
                        .label("Icon")
                        .halign(Align::Start)
                        .build(),
                )
                .css_classes(["flat", "icon_chooser_menu_button"])
                .build();

            let description_button = gtk::Button::builder()
                .child(
                    &gtk::Label::builder()
                        .label("Description")
                        .halign(Align::Start)
                        .build(),
                )
                .css_classes(["flat", "icon_chooser_menu_button"])
                .build();

            let remove_button = gtk::Button::builder()
                .child(
                    &gtk::Label::builder()
                        .label("Remove")
                        .halign(Align::Start)
                        .build(),
                )
                .css_classes(["flat", "icon_chooser_menu_button"])
                .build();

            let options_box = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .build();
            options_box.append(&file_button);
            options_box.append(&icon_button);
            options_box.append(&remove_button);
            options_box.append(&description_button);

            let nav_view = adw::NavigationView::builder().build();

            let options_nav_page = adw::NavigationPage::builder()
                .title("Icon Chooser")
                .child(&options_box)
                .build();

            nav_view.push(&options_nav_page);

            let obj = self.obj();

            // Remove
            remove_button.set_detailed_action_name("desktop_file_group.remove_entry('Icon')");
            remove_button.connect_clicked(clone!(
                #[weak]
                popover,
                move |_btn| {
                    popover.set_visible(false);
                }
            ));

            // Description
            description_button.set_detailed_action_name("desktop_file_group.show_entry_info('Icon')");
            description_button.connect_clicked(clone!(
                #[weak]
                popover,
                move |_btn| {
                    popover.set_visible(false);
                }
            ));

            // File
            file_button.connect_clicked(clone!(
                #[weak]
                obj,
                #[weak]
                popover,
                move |button| {
                    popover.set_visible(false);

                    let filter = FileFilter::new();
                    filter.set_name(Some("Image File"));
                    filter.add_mime_type("image/png");
                    filter.add_mime_type("image/jpeg");
                    filter.add_mime_type("image/svg+xml");
                    filter.add_mime_type("image/bmp");
                    filter.add_mime_type("image/webp");
                    let dialog = FileDialog::builder()
                        .title("Choose Icon")
                        .default_filter(&filter)
                        .build();

                    dialog.open(
                        button
                            .root()
                            .map(|root| root.downcast::<gtk::Window>().unwrap())
                            .as_ref(),
                        Some(&Cancellable::new()),
                        clone!(
                            #[weak]
                            obj,
                            move |res| {
                                if let Ok(res) = res {
                                    obj.set_text(res.path().unwrap().to_str().unwrap());
                                    obj.activate();
                                }
                            }
                        ),
                    );
                }
            ));

            // Icons
            let empty_arr: &[Expression] = &[];
            let string_filter_expr = ClosureExpression::new::<String>(
                empty_arr,
                closure_local!(|entry: Option<StringObject>| {
                    entry.map(|ent| ent.string()).unwrap_or_default()
                }),
            );

            let string_filter = StringFilter::builder()
                .match_mode(StringFilterMatchMode::Substring)
                .expression(string_filter_expr)
                .ignore_case(true)
                .build();
            self.icon_search_filter.set(string_filter);

            let grid_view = self.make_icon_grid();
            grid_view.connect_activate(clone!(
                #[weak]
                obj,
                move |grid_view, position| {
                    let model = grid_view.model().expect("No model present");
                    let string_object = model
                        .item(position)
                        .and_downcast::<StringObject>()
                        .expect("Needs to be StringObject");

                    obj.set_text(&string_object.string());
                    obj.activate();
                }
            ));

            let scrolled_window = ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&grid_view)
                .width_request(0)
                .build();

            let grid_revealer = Revealer::builder()
                .child(&scrolled_window)
                .transition_type(RevealerTransitionType::SlideLeft)
                .transition_duration(250)
                .build();

            let search_entry = SearchEntry::new();
            search_entry.set_key_capture_widget(Some(&scrolled_window));
            search_entry.connect_search_changed(clone!(
                #[weak(rename_to = filter)]
                self.icon_search_filter,
                move |search_entry| {
                    filter.borrow().set_search(Some(&search_entry.text()));
                    filter.borrow().search();
                }
            ));

            let search_entry_revealer = Revealer::builder()
                .child(&search_entry)
                .transition_type(RevealerTransitionType::SlideLeft)
                .transition_duration(250)
                .build();

            let header_bar = adw::HeaderBar::builder()
                .show_title(true)
                .show_end_title_buttons(false)
                .title_widget(&search_entry_revealer)
                .width_request(0)
                .build();

            let toolbar_view = adw::ToolbarView::new();
            toolbar_view.add_top_bar(&header_bar);
            toolbar_view.set_content(Some(&grid_revealer));

            let icons_nav_page = adw::NavigationPage::builder()
                .title("App Icons")
                .child(&toolbar_view)
                .build();

            // Popover animations
            let anim_width_target = adw::PropertyAnimationTarget::new(popover, "width_request");
            let anim_height_target = adw::PropertyAnimationTarget::new(popover, "height_request");
            let anim_width = adw::TimedAnimation::builder()
                .duration(POPOVER_ANIM_DURATION)
                .value_from(POPOVER_SIZE_SMALL)
                .value_to(POPOVER_SIZE_LARGE)
                .target(&anim_width_target)
                .widget(popover)
                .build();

            let anim_height = adw::TimedAnimation::builder()
                .duration(POPOVER_ANIM_DURATION)
                .value_from(POPOVER_SIZE_SMALL)
                .value_to(POPOVER_SIZE_LARGE)
                .target(&anim_height_target)
                .widget(popover)
                .build();

            anim_width.connect_done(clone!(
                #[weak]
                search_entry_revealer,
                #[weak]
                grid_revealer,
                move |anim| {
                    if !anim.is_reverse() {
                        search_entry_revealer.set_reveal_child(true);
                        grid_revealer.set_reveal_child(true);
                    }
                }
            ));

            nav_view.connect_popped(clone!(
                #[weak]
                search_entry_revealer,
                #[weak]
                grid_revealer,
                #[weak]
                anim_width,
                #[weak]
                anim_height,
                move |_, _| {
                    search_entry_revealer.set_reveal_child(false);
                    grid_revealer.set_reveal_child(false);
                    anim_width.set_easing(adw::Easing::EaseInQuad);
                    anim_width.set_reverse(true);
                    anim_width.play();
                    anim_height.set_easing(adw::Easing::EaseInQuad);
                    anim_height.set_reverse(true);
                    anim_height.play();
                }
            ));

            icon_button.connect_clicked(clone!(
                #[weak]
                nav_view,
                #[strong]
                icons_nav_page,
                move |_| {
                    nav_view.push(&icons_nav_page);

                    anim_width.set_easing(adw::Easing::EaseOutQuad);
                    anim_width.set_reverse(false);
                    anim_width.play();
                    anim_height.set_easing(adw::Easing::EaseOutQuad);
                    anim_height.set_reverse(false);
                    anim_height.play();
                }
            ));

            nav_view.into()
        }

        fn make_icon_grid(&self) -> GridView {
            let icon_theme = IconTheme::for_display(&self.obj().display());
            let binding = icon_theme.icon_names();

            let mut icon_names: Vec<_> = binding.iter().map(|name| name.as_str()).collect();
            icon_names.sort();

            let model = StringList::new(&icon_names);

            let factory = SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let image = gtk::Image::builder()
                    .pixel_size(32)
                    .css_classes(["lowres-icon"])
                    .build();

                list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem")
                    .set_child(Some(&image));
            });

            factory.connect_bind(move |_, list_item| {
                let string_object = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem")
                    .item()
                    .and_downcast::<StringObject>()
                    .expect("Needs to be StringObject");

                let image = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem")
                    .child()
                    .and_downcast::<Image>()
                    .expect("Needs to be Image");

                let icon_name = string_object.string();
                image.set_icon_name(Some(&icon_name));
                image.set_tooltip_text(Some(&icon_name));
            });

            let filter_model =
                FilterListModel::new(Some(model), Some(self.icon_search_filter.borrow().clone()));
            let selection_model = NoSelection::new(Some(filter_model));

            GridView::builder()
                .model(&selection_model)
                .factory(&factory)
                .single_click_activate(true)
                .build()
        }
    }
}

glib::wrapper! {
    pub struct IconEntryRow(ObjectSubclass<imp::IconEntryRow>)
        @extends adw::EntryRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget, gtk::Editable;
}

impl IconEntryRow {
    pub fn new(title: &str, text: &str) -> IconEntryRow {
        glib::Object::builder()
            .property("title", title)
            .property("text", text)
            .build()
    }

    pub fn update_icon(&self) {
        let imp = self.imp();
        imp.icon_image.borrow_mut().set_from_gicon(&self.gicon());
    }
}

impl ToGIcon for IconEntryRow {
    fn icon_string(&self) -> Option<String> {
        Some(self.text().to_string())
    }
}
