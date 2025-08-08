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

mod add_action_dialog;
mod add_locale_dialog;
mod close_confirm_dialog;
pub mod desktop_entry_ext;
mod desktop_file_group;
mod known_entries;
mod languages;
mod string_entry_row;
mod util;

use std::{borrow::Borrow, path::Path, rc::Rc};

use adw::{prelude::*, NavigationPage};
use gtk::{
    gio,
    glib::{self, subclass::types::ObjectSubclassIsExt},
    Widget,
};

use self::{
    desktop_entry_ext::DesktopEntryExt, desktop_file_group::DesktopFileGroup, imp::DesktopEntryCell,
};

mod imp {
    use adw::subclass::prelude::*;

    use gtk::gio::Cancellable;
    use gtk::glib::property::PropertySet;
    use gtk::glib::{clone, closure, closure_local, Object, Propagation, SignalHandlerId};
    use gtk::PropertyExpression;
    use notify::{INotifyWatcher, RecursiveMode, Watcher};
    use std::borrow::Borrow;
    use std::cell::Cell;

    use std::fs;
    use std::path::Path;
    use std::rc::Rc;
    use std::{cell::RefCell, path::PathBuf};

    use adw::{prelude::*, NavigationPage, NavigationView};
    use freedesktop_desktop_entry::DesktopEntry;
    use gtk::glib::subclass::InitializingObject;
    use gtk::{
        glib::{self, Properties},
        subclass::widget::{CompositeTemplateClass, CompositeTemplateInitializingExt, WidgetImpl},
        CompositeTemplate,
    };

    use crate::desktop_file_view::desktop_entry_ext::{DesktopEntryExt, DEFAULT_LOCALE, NO_LOCALE};

    use crate::window::file_entry::ToGIcon;

    use super::add_action_dialog::AddActionDialog;
    use super::add_locale_dialog::AddLocaleDialog;
    use super::close_confirm_dialog::show_close_confirm_dialog;
    use super::desktop_file_group::DesktopFileGroup;
    use super::languages::LANGUAGES_LOCALE_MAP;

    pub type DesktopEntryCell = RefCell<DesktopEntry>;

    #[derive(CompositeTemplate, Default, Properties)]
    #[template(resource = "/org/argoware/desktop_file_editor/desktop_file_view.ui")]
    #[properties(wrapper_type = super::DesktopFileView)]
    pub struct DesktopFileView {
        #[template_child]
        pub desktop_entry_group: TemplateChild<DesktopFileGroup>,

        #[template_child]
        pub additional_groups: TemplateChild<gtk::Box>,

        #[template_child]
        pub image: TemplateChild<gtk::Image>,

        #[template_child]
        pub locale_dropdown: TemplateChild<gtk::DropDown>,

        #[template_child]
        reload_bar: TemplateChild<gtk::Revealer>,

        #[property(get, set, construct)]
        path: RefCell<PathBuf>,

        #[property(get, set, construct)]
        parent_navigation_view: RefCell<adw::NavigationView>,

        #[property(get, set)]
        content_changed: Cell<bool>,

        #[property(get, set, nullable)]
        locale: RefCell<Option<String>>,

        pub desktop_entry: RefCell<Option<Rc<DesktopEntryCell>>>,

        back_confirm_handler: Cell<Option<SignalHandlerId>>,
        close_confirm_handler: RefCell<Option<SignalHandlerId>>,
        window: RefCell<Option<adw::ApplicationWindow>>,

        file_watcher: RefCell<Option<INotifyWatcher>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DesktopFileView {
        const NAME: &'static str = "DesktopFileView";
        type Type = super::DesktopFileView;
        type ParentType = NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            DesktopFileGroup::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DesktopFileView {
        fn dispose(&self) {
            self.dispose_template();
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.desktop_entry_group
                .set_desktop_file_view(Some(self.obj().downgrade()));

            self.init_locale_dropdown();
            self.reset();

            if let Err(e) = self.init_file_watcher() {
                eprintln!("Failed to initialize file watcher: {e}");
            };

            if let Err(e) = self.start_file_watcher() {
                eprintln!("Failed to watch desktop file for changes: {e}");
            };
            let back_confirm_handler = self
                .parent_navigation_view
                .borrow()
                .connect_popped(Self::back_confirm_callback);

            self.back_confirm_handler.set(Some(back_confirm_handler));

            self.obj().connect_locale_notify(|desktop_file_view| {
                desktop_file_view.update_locale();
            });
        }
    }

    impl WidgetImpl for DesktopFileView {
        fn realize(&self) {
            self.parent_realize();

            if self.close_confirm_handler.borrow().is_none() {
                let desktop_file_view = ObjectExt::downgrade(&self.obj().clone());

                let close_confirm_handler = self.window().connect_close_request(move |win| {
                    let desktop_file_view = if let Some(upgraded) = desktop_file_view.upgrade() {
                        upgraded
                    } else {
                        eprintln!("DesktopFileView ceased to exist but the relative window's close-request signal is still connected, this is a bug, please report it.");
                        return Propagation::Proceed;
                    };

                    let imp = desktop_file_view.imp();
                    if !imp.content_changed.get() {
                        // If nothing changed there is no need for confirmation
                        imp.disconnect_close_confirm_handlers();
                        return Propagation::Proceed;
                    }

                    show_close_confirm_dialog(
                        win,
                        || {},
                        clone!(
                            #[weak]
                            win,
                            #[weak]
                            desktop_file_view,
                            move || {
                                // On discard
                                desktop_file_view.imp().disconnect_close_confirm_handlers();
                                win.close();
                            }
                        ),
                    );

                    Propagation::Stop
                });
                self.close_confirm_handler.set(Some(close_confirm_handler));
            }
        }
    }
    impl NavigationPageImpl for DesktopFileView {}

    #[gtk::template_callbacks]
    impl DesktopFileView {
        #[template_callback]
        fn on_save_button_clicked(&self, button: &gtk::Button) {
            {
                let borrow = self.desktop_entry.borrow();
                let content: &RefCell<DesktopEntry> = borrow.as_ref().unwrap().borrow();
                let content = content.borrow().to_sorted_entry_string();

                let path = self.path.borrow().to_path_buf();

                if let Err(e) = self.stop_file_watcher() {
                    eprintln!("Failed to stop file watcher before saving: {e}");
                }

                trash::delete(&path).expect("Failed to trash original file");
                fs::write(&path, content).expect("Failed to write file");
                println!("File {} written!", path.to_string_lossy());

                if let Err(e) = self.start_file_watcher() {
                    eprintln!("Failed to restart file watcher: {e}");
                }

                button.set_sensitive(false);
            }

            self.reset();
        }

        #[template_callback]
        fn on_reset_button_clicked(&self, _: &gtk::Button) {
            self.reset();
        }

        #[template_callback]
        fn on_reload_button_clicked(&self, _: &gtk::Button) {
            self.reset();
        }

        #[template_callback]
        fn on_add_locale_button_clicked(&self, button: &gtk::Button) {
            let dialog = AddLocaleDialog::new();
            dialog
                .property_expression_weak("locale")
                .chain_closure::<bool>(closure_local!(
                    #[weak(rename_to=this)]
                    self,
                    move |_: Option<Object>, locale: &str| { this.obj().locale_exists(locale) }
                ))
                .bind(&dialog, "locale_exists", Object::NONE);

            dialog.clone().choose(
                button,
                Cancellable::NONE,
                clone!(
                    #[weak]
                    dialog,
                    #[weak(rename_to=this)]
                    self,
                    move |response| {
                        if response == "add" {
                            let locale = dialog.locale();
                            this.obj().add_locale(&locale);
                        }
                    }
                ),
            );
        }

        #[template_callback]
        fn on_add_action_button_clicked(&self, button: &gtk::Button) {
            let dialog = AddActionDialog::new();
            dialog.clone().choose(
                button,
                Cancellable::NONE,
                clone!(
                    #[weak(rename_to=this)]
                    self,
                    #[weak]
                    dialog,
                    move |response| {
                        if response == "add" {
                            let action_name = dialog.action();
                            this.obj().add_action(&action_name);
                        }
                    }
                ),
            );
        }

        fn load_desktop_entry_file(&self) {
            let path = self.path.clone().into_inner();
            let desktop_entry = DesktopEntry::from_path(path.clone(), None::<&[&str]>)
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to create desktop entry from path: {}",
                        path.to_string_lossy()
                    )
                });
            self.desktop_entry
                .set(Some(Rc::new(RefCell::new(desktop_entry))));
        }

        fn reset(&self) {
            // Empty content list
            self.reload_bar.clone().set_reveal_child(false);

            let obj = self.obj();
            obj.set_content_changed(false);

            // Reload file
            self.load_desktop_entry_file();

            // The scope is necessary to avoid BorrowMutError, to make sure the borrowed desktop
            // entry is dropped.
            // this happens because a populate causes the dropdown to
            // automatically select the Default locale, which causes callbacks to be called that
            // might themselves try to borrow the entry.
            {
                let some_entry = self.desktop_entry.borrow();
                let desktop_entry_rc = some_entry.as_ref().unwrap();
                let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
                let desktop_entry = desktop_entry_cell.borrow();

                let name = desktop_entry.name(&NO_LOCALE);
                obj.set_title(&name.unwrap_or_else(|| "No Name".into()));

                self.image.set_from_gicon(&desktop_entry.gicon());
            }

            self.desktop_entry_group.populate();
            self.reset_additional_groups();
            self.populate_dropdown();
        }

        pub fn reset_additional_groups(&self) {
            // Remove all children in additional groups
            let mut child = self.additional_groups.first_child();
            while child.is_some() {
                self.additional_groups.remove(&child.unwrap());
                child = self.additional_groups.first_child();
            }

            // Repopulate
            let group_names: Vec<String> = {
                let some_entry = self.desktop_entry.borrow();
                let desktop_entry_cell: &DesktopEntryCell = some_entry.as_ref().unwrap().borrow();
                let desktop_entry = desktop_entry_cell.borrow();

                desktop_entry
                    .groups
                    .0
                    .keys()
                    .filter(|&group_name| group_name != "Desktop Entry")
                    .cloned()
                    .collect()
            };

            for group_name in group_names {
                let desktop_file_group = DesktopFileGroup::new(&group_name, true);
                desktop_file_group.set_desktop_file_view(Some(self.obj().downgrade()));
                desktop_file_group.populate();
                self.additional_groups.append(&desktop_file_group);
            }
        }

        fn init_locale_dropdown(&self) {
            // Expression for search feature as well as the display string
            self.locale_dropdown.set_expression(Some(
                PropertyExpression::new(
                    gtk::StringObject::static_type(),
                    gtk::Expression::NONE,
                    "string",
                )
                .chain_closure::<String>(closure!(
                    |_: Option<glib::Object>, locale: &str| {
                        let language = LANGUAGES_LOCALE_MAP.get(locale).unwrap_or(&"");
                        format!("[{locale}] {language}")
                    }
                )),
            ));

            self.locale_dropdown.connect_selected_item_notify(clone!(
                #[weak(rename_to=this)]
                self,
                move |dropdown| {
                    let item = dropdown
                        .selected_item()
                        .expect("Failed to get selected item")
                        .downcast::<gtk::StringObject>()
                        .expect("Selected item is not a StringObject");

                    println!("Selected locale: {}", item.string());

                    let locale = item.string();
                    let locale = if locale == DEFAULT_LOCALE {
                        None
                    } else {
                        Some(locale.to_string())
                    };

                    this.obj().set_locale(locale);
                }
            ));
        }

        fn populate_dropdown(&self) {
            // For the same reason as above, we drop the borrow before setting the model
            let string_list = {
                let borrowed_entry = self.desktop_entry.borrow();
                let desktop_entry_cell: &DesktopEntryCell =
                    borrowed_entry.as_ref().unwrap().borrow();
                let desktop_entry = desktop_entry_cell.borrow();

                let locales = desktop_entry.locales();
                let mut locales: Vec<&str> = locales.iter().map(|s| s.borrow()).collect();

                locales.insert(0, DEFAULT_LOCALE);
                gtk::StringList::new(&locales[..])
            };

            self.locale_dropdown.set_model(Some(&string_list));
        }

        fn init_file_watcher(&self) -> Result<(), notify::Error> {
            let path_ref = self.path.borrow();

            let (sender, receiver) = async_channel::bounded(1);

            let path_buf = path_ref.clone();
            let file_watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    match res {
                        Ok(event) => {
                            println!("{event:?}");
                            if event.paths.contains(&path_buf) && event.kind.is_modify() {
                                // This could fail if the channel is full, but we don't care, as we
                                // only need one message to go through
                                let _ = sender.try_send(true);
                            }
                        }
                        Err(e) => eprintln!("file watch error: {e:?}"),
                    }
                })?;

            self.file_watcher.set(Some(file_watcher));

            let reload_bar = self.reload_bar.clone();
            glib::spawn_future_local(clone!(
                #[weak]
                reload_bar,
                async move {
                    while let Ok(reveal_reload_bar) = receiver.recv().await {
                        reload_bar.set_reveal_child(reveal_reload_bar);
                    }
                }
            ));

            Ok(())
        }

        fn perform_watcher_action<F>(&self, f: F) -> Result<(), notify::Error>
        where
            F: FnOnce(&mut INotifyWatcher, &Path) -> Result<(), notify::Error>,
        {
            // In order to properly watch the file regardless of what file editors do, we
            // watch the parent directory and only act on the file we want
            let path_borrow = self.path.borrow();
            let path = path_borrow.as_path();
            let parent_path = path.parent().expect("Failed to get file's parent path");

            let res = self
                .file_watcher
                .borrow_mut()
                .as_mut()
                .map(|watcher| f(watcher, parent_path));

            match res {
                Some(result) => Ok(result?),
                None => Ok(()),
            }
        }

        fn start_file_watcher(&self) -> Result<(), notify::Error> {
            self.perform_watcher_action(|watcher, parent_path| {
                watcher.watch(parent_path, RecursiveMode::NonRecursive)
            })
        }

        fn stop_file_watcher(&self) -> Result<(), notify::Error> {
            self.perform_watcher_action(|watcher, parent_path| watcher.unwatch(parent_path))
        }

        fn window(&self) -> adw::ApplicationWindow {
            let win = self.window.borrow().clone();
            match win {
                Some(win) => win,
                None => {
                    let win = self
                        .obj()
                        .root()
                        .expect("No Root")
                        .downcast::<adw::ApplicationWindow>()
                        .expect("Root is not a window");
                    self.window.set(Some(win.clone()));
                    win
                }
            }
        }

        fn disconnect_close_confirm_handlers(&self) {
            let back_confirm_handler = self.back_confirm_handler.replace(None);
            if let Some(back_confirm_handler) = back_confirm_handler {
                self.parent_navigation_view
                    .borrow()
                    .disconnect(back_confirm_handler);
            }

            let close_confirm_handler = self.close_confirm_handler.replace(None);
            if let Some(close_confirm_handler) = close_confirm_handler {
                self.window().disconnect(close_confirm_handler);
            }
        }

        fn back_confirm_callback(view: &NavigationView, page: &NavigationPage) {
            let desktop_file_view: super::DesktopFileView = page.clone().downcast().unwrap();

            let imp = desktop_file_view.imp();
            if !imp.content_changed.get() {
                // If nothing changed there is no need for confirmation
                imp.disconnect_close_confirm_handlers();
                return;
            }

            show_close_confirm_dialog(
                view,
                clone!(
                    #[weak]
                    view,
                    #[strong]
                    desktop_file_view,
                    move || {
                        // On cancel
                        view.push(&desktop_file_view);
                    }
                ),
                clone!(
                    #[strong]
                    desktop_file_view,
                    move || {
                        // On discard
                        let imp = desktop_file_view.imp();
                        imp.disconnect_close_confirm_handlers();
                    }
                ),
            );
        }
    }
}

glib::wrapper! {
    pub struct DesktopFileView(ObjectSubclass<imp::DesktopFileView>)
        @extends NavigationPage, Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DesktopFileView {
    pub fn new(parent_navigation_view: adw::NavigationView, path: &Path) -> DesktopFileView {
        glib::Object::builder()
            .property("path", path.to_path_buf())
            .property("parent_navigation_view", parent_navigation_view)
            .build()
    }

    pub fn desktop_entry(&self) -> Rc<DesktopEntryCell> {
        self.imp().desktop_entry.borrow().as_ref().unwrap().clone()
    }

    pub fn set_image_icon(&self, icon: &impl IsA<gio::Icon>) {
        self.imp().image.set_from_gicon(icon);
    }

    pub fn remove_group(&self, group: &DesktopFileGroup) {
        let desktop_entry_rc = self.desktop_entry();
        let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
        let mut desktop_entry = desktop_entry_cell.borrow_mut();

        desktop_entry.remove_group(group.name());
        self.imp().additional_groups.remove(group);
        self.set_content_changed(true);
    }

    fn locale_exists(&self, locale: &str) -> bool {
        // Here we check the list of locales in the dropdown instead of the data in the entries,
        // because adding a locale does not modify the actual data store,
        // which might lead to finding that a locale does not exist, even if it was alrteady added to the
        // dropdown.

        let dropdown = &self.imp().locale_dropdown;
        if let Some(model) = dropdown.model() {
            let string_list = model
                .downcast::<gtk::StringList>()
                .expect("Dropdown model is not StringList");

            for pos in 0..string_list.n_items() {
                let s = string_list.string(pos);
                if let Some(s) = s {
                    if s == locale {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn add_locale(&self, locale: &str) {
        if locale.is_empty() {
            return;
        }

        // Only add a new dropdown item if the locale does not already exist
        if !self.locale_exists(locale) {
            let dropdown = &self.imp().locale_dropdown;
            if let Some(model) = dropdown.model() {
                let string_list = model
                    .downcast::<gtk::StringList>()
                    .expect("Dropdown model is not StringList");
                let new_locale_idx = string_list.n_items();
                string_list.append(locale);
                dropdown.set_selected(new_locale_idx);
            }
        }

        self.set_locale(Some(locale));
    }

    pub fn add_action(&self, action_name: &str) {
        if action_name.is_empty() {
            return;
        }

        {
            // Desktop Entry borrow should not escape this block
            let desktop_entry_rc = self.desktop_entry();
            let desktop_entry_cell: &DesktopEntryCell = desktop_entry_rc.borrow();
            let mut desktop_entry = desktop_entry_cell.borrow_mut();
            desktop_entry.add_action(action_name);
            let group_name = format!("Desktop Action {action_name}");
            desktop_entry.add_entry(group_name.clone(), "Name".to_string());
            desktop_entry.add_entry(group_name, "Exec".to_string());
        }

        self.imp().reset_additional_groups();
        self.set_content_changed(true);
    }

    fn update_locale(&self) {
        let locale = self.locale();
        let locale_ref = locale.as_deref();
        let imp = self.imp();

        imp.desktop_entry_group.change_locale(locale_ref);

        let mut child = imp.additional_groups.first_child();
        while child.is_some() {
            let group = child
                .unwrap()
                .downcast::<DesktopFileGroup>()
                .expect("Child is not DesktopFileGroup");

            group.change_locale(locale_ref);
            child = group.next_sibling();
        }

        // Also select the appropriate item in the dropdown if not already selected, this is
        // necessary when programmatically changing the selected locale
        let dropdown = &self.imp().locale_dropdown;
        if let Some(model) = dropdown.model() {
            let string_list = model
                .downcast::<gtk::StringList>()
                .expect("Dropdown model is not StringList");

            for pos in 0..string_list.n_items() {
                let s = string_list.string(pos);
                if s.as_deref() == locale_ref {
                    if dropdown.selected() != pos {
                        dropdown.set_selected(pos);
                    }
                    break;
                }
            }
        }
    }
}
