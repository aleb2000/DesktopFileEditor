mod entry_filter;
pub(crate) mod file_entry;
mod list_entry;
mod sliding_search_entry;

use gtk::{
    gio::{self, ListStore},
    glib,
    subclass::prelude::*,
};

use crate::application::DMApplication;

mod imp {
    use std::cell::Cell;
    use std::cell::RefCell;

    use std::fs;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::Duration;

    use adw::gio;
    use adw::glib;
    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use either::Either;
    use gtk::gio::ListStore;
    use gtk::gio::PropertyAction;
    use gtk::glib::property::PropertyGet;
    use gtk::glib::property::PropertySet;
    use gtk::glib::Properties;
    use gtk::glib::{
        clone, closure, closure_local, object_subclass, subclass::InitializingObject, Object,
    };
    use gtk::EveryFilter;
    use gtk::{
        template_callbacks, ClosureExpression, CompositeTemplate, CustomSorter, Expression,
        FilterListModel, ListItem, ListView, NoSelection, SignalListItemFactory, SortListModel,
        StringFilter, StringFilterMatchMode, Widget,
    };
    use notify::INotifyWatcher;
    use notify::Watcher;
    use notify_debouncer_full::DebounceEventResult;
    use notify_debouncer_full::Debouncer;
    use notify_debouncer_full::FileIdMap;

    use crate::desktop_file_view::DesktopFileView;
    use crate::window::file_entry::ToGIcon;
    use crate::window::file_entry::ValidityStatus;

    use super::entry_filter::EntryFilter;
    use super::file_entry::FileEntry;
    use super::list_entry::ListEntry;
    use super::sliding_search_entry::SlidingSearchEntry;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/org/argoware/desktop_manager/window.ui")]
    #[properties(wrapper_type = super::DMWindow)]
    pub struct DMWindow {
        #[template_child]
        pub search_entry: TemplateChild<SlidingSearchEntry>,

        #[template_child]
        pub entries_list: TemplateChild<ListView>,

        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,

        #[property(get, set, construct)]
        pub additional_search_paths: RefCell<Vec<String>>,

        #[property(get, set, construct)]
        pub ignore_default_paths: Cell<bool>,

        pub entries: RefCell<Option<ListStore>>,

        search_filter: Rc<RefCell<StringFilter>>,
        entry_filter: Rc<RefCell<EntryFilter>>,

        pub app_paths_watcher: RefCell<Option<Debouncer<INotifyWatcher, FileIdMap>>>,
    }

    #[object_subclass]
    impl ObjectSubclass for DMWindow {
        const NAME: &'static str = "DMWindow";
        type Type = super::DMWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            SlidingSearchEntry::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DMWindow {
        fn dispose(&self) {
            self.dispose_template();
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.init_list();
            self.search_entry
                .search_entry()
                .connect_search_changed(clone!(
                    #[weak(rename_to = filter)]
                    self.search_filter,
                    move |search_entry| {
                        filter.borrow().set_search(Some(&search_entry.text()));
                        filter.borrow().search();
                    }
                ));

            self.search_entry
                .set_key_capture_widget(Some(self.obj().clone().upcast::<Widget>()));

            let obj = self.obj();
            self.entry_filter.get(|filter| {
                let filter_hidden_action = PropertyAction::new("filter-hidden", filter, "hidden");
                let filter_invalid_action =
                    PropertyAction::new("filter-invalid", filter, "invalid");
                let filter_only_show_selected_action=
                    PropertyAction::new("filter-only-show-selected", filter, "only-show-selected");
                obj.add_action(&filter_hidden_action);
                obj.add_action(&filter_invalid_action);
                obj.add_action(&filter_only_show_selected_action);
            });
        }
    }

    impl WidgetImpl for DMWindow {}
    impl WindowImpl for DMWindow {}
    impl ApplicationWindowImpl for DMWindow {}
    impl AdwApplicationWindowImpl for DMWindow {}

    #[template_callbacks]
    impl DMWindow {
        #[template_callback]
        fn on_listview_activate(&self, position: u32, list_view: ListView) {
            let model = list_view.model().expect("The listview has no model");
            let item: FileEntry = model
                .item(position)
                .and_downcast()
                .expect("The item is not an entry");

            if item.path().exists() {
                let nav_view = self.navigation_view.clone();
                let desktop_file_view = DesktopFileView::new(nav_view, &item.path());
                self.navigation_view.push(&desktop_file_view);
            }
        }

        fn init_list(&self) {
            self.load_entries();
            if let Err(e) = self.watch_entries_dirs() {
                eprintln!("Failed to watch application directories: {}", e);
                eprintln!("The list will not be updated on changes");
            }

            let factory = SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let entry = ListEntry::new();
                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Should be ListItem");
                list_item.set_child(Some(&entry));

                list_item
                    .property_expression("item")
                    .chain_property::<FileEntry>("name")
                    .bind(&entry.name_label(), "label", Widget::NONE);

                list_item
                    .property_expression("item")
                    .chain_property::<FileEntry>("path")
                    .bind(&entry.path_label(), "label", Widget::NONE);

                list_item
                    .property_expression("item")
                    .chain_closure::<gio::Icon>(closure!(
                        |_: Option<Object>, entry: Option<&FileEntry>| {
                            entry.map_or_else(FileEntry::default_exec_gicon, |entry| entry.gicon())
                        }
                    ))
                    .bind(&entry.icon_image(), "gicon", Widget::NONE);

                list_item
                    .property_expression("item")
                    .chain_property::<FileEntry>("should-show")
                    .bind(&entry, "should-show", Widget::NONE);

                list_item
                    .property_expression("item")
                    .chain_property::<FileEntry>("validity-status")
                    .chain_closure::<bool>(closure!(
                        |_: Option<Object>, status: &ValidityStatus| { !status.is_valid() }
                    ))
                    .bind(&entry.invalid_marker(), "visible", Widget::NONE);

                list_item
                    .property_expression("item")
                    .chain_property::<FileEntry>("validity-status")
                    .chain_closure::<String>(closure!(
                        |_: Option<Object>, status: &ValidityStatus| { status.error_string() }
                    ))
                    .bind(&entry.invalid_marker(), "tooltip-text", Widget::NONE);
            });

            let sorter = CustomSorter::new(move |obj1, obj2| {
                let obj1 = obj1
                    .downcast_ref::<FileEntry>()
                    .expect("Should be EntryObj");
                let obj2 = obj2
                    .downcast_ref::<FileEntry>()
                    .expect("Should be EntryObj");
                obj1.name().cmp(&obj2.name()).into()
            });

            // Setup search filter
            let empty_arr: &[Expression] = &[];
            let entry_key_expr = ClosureExpression::new::<String>(
                empty_arr,
                closure_local!(|entry: Option<FileEntry>| {
                    entry.map(|ent| ent.search_key()).unwrap_or_default()
                }),
            );

            *self.search_filter.borrow_mut() = StringFilter::builder()
                .match_mode(StringFilterMatchMode::Substring)
                .expression(entry_key_expr)
                .ignore_case(true)
                .build();

            self.entry_filter.set(EntryFilter::default());

            let multi_filter = EveryFilter::new();
            multi_filter.append(self.search_filter.borrow().clone());
            multi_filter.append(self.entry_filter.borrow().clone());

            let filter_model = FilterListModel::new(Some(self.obj().entries()), Some(multi_filter));
            let sort_model = SortListModel::new(Some(filter_model), Some(sorter));
            let selection_model = NoSelection::new(Some(sort_model));

            self.entries_list.set_factory(Some(&factory));
            self.entries_list.set_model(Some(&selection_model));
        }

        fn load_entries(&self) {
            let app_paths = self.application_paths();

            let mut store = ListStore::new::<FileEntry>();

            for dir in app_paths {
                println!("Scanning {dir:?}");

                let entries = match find_all_desktop_files(&dir) {
                    Ok(files) => Either::Left(files.into_iter().filter_map(|path| {
                        let file_entry = FileEntry::from_path(&path);
                        if file_entry.is_err() {
                            eprintln!(
                                "Failed to create file entry for {}: {}",
                                path.to_string_lossy(),
                                file_entry.as_ref().unwrap_err()
                            );
                        }
                        file_entry.ok()
                    })),
                    Err(e) => {
                        eprintln!("Failed to scan: {}", e);
                        Either::Right(std::iter::empty())
                    }
                };
                store.extend(entries);
            }

            self.entries.set(Some(store));
        }

        fn watch_entries_dirs(&self) -> Result<(), notify::Error> {
            let (sender, receiver) = async_channel::unbounded();
            let mut debouncer = notify_debouncer_full::new_debouncer(
                Duration::from_secs(1),
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => events.into_iter().for_each(|event| {
                        if event.kind.is_remove()
                            || event.kind.is_modify()
                            || event.kind.is_create()
                        {
                            for path in event.paths.iter() {
                                if let Err(e) = sender.send_blocking(path.clone()) {
                                    eprintln!("Error sending application list watch update: {}", e);
                                }
                            }
                        }
                    }),
                    Err(errors) => errors.iter().for_each(|error| println!("{error:?}")),
                },
            )
            .unwrap();

            let app_paths = self.application_paths();
            for path in app_paths {
                println!("Watching {}", path.to_string_lossy());
                let res = debouncer
                    .watcher()
                    .watch(&path, notify::RecursiveMode::Recursive);

                if let Err(e) = res {
                    eprintln!("Failed to watch: {}", e);
                    continue;
                }

                debouncer
                    .cache()
                    .add_root(&path, notify::RecursiveMode::Recursive);
            }
            self.app_paths_watcher.set(Some(debouncer));

            let entries = self.obj().entries();
            fn find_entry(entries: &ListStore, path: &Path) -> Option<(u32, FileEntry)> {
                for (i, entry) in entries.iter::<FileEntry>().enumerate() {
                    if let Ok(entry) = entry {
                        if entry.path() == path {
                            return Some((i as u32, entry));
                        }
                    }
                }
                None
            }

            glib::spawn_future_local(clone!(async move {
                while let Ok(path) = receiver.recv().await {
                    if path.exists() {
                        match find_entry(&entries, &path) {
                            Some((i, entry)) => {
                                // Update entry
                                if let Err(e) = entry.update() {
                                    eprintln!(
                                        "Failed to decode entry on update {}: {}",
                                        path.to_string_lossy(),
                                        e
                                    );
                                    entries.remove(i);
                                }
                            }
                            None => {
                                // Create entry
                                match FileEntry::from_path(&path) {
                                    Ok(entry) => entries.append(&entry),
                                    Err(e) => {
                                        eprintln!(
                                            "Entry creation failed {}: {}",
                                            path.to_string_lossy(),
                                            e
                                        )
                                    }
                                }
                            }
                        }
                    } else {
                        // Remove entry
                        if let Some((i, _)) = find_entry(&entries, &path) {
                            entries.remove(i);
                        }
                    }
                }
            }));

            Ok(())
        }

        fn application_paths(&self) -> impl Iterator<Item = PathBuf> {
            let application_paths = if self.ignore_default_paths.get() {
                Either::Left(std::iter::empty())
            } else {
                Either::Right(freedesktop_desktop_entry::default_paths())
            };

            // Add additional search paths
            let additional_search_paths = self
                .obj()
                .additional_search_paths()
                .into_iter()
                .map(PathBuf::from);
            application_paths.chain(additional_search_paths)
        }
    }

    /// Recursively find all desktop files in a given directory
    fn find_all_desktop_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(fs::read_dir(dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if entry.file_type().ok()?.is_dir() {
                    Some(find_all_desktop_files(&path).ok()?)
                } else if path.extension()? == "desktop" {
                    Some(vec![path])
                } else {
                    None
                }
            })
            .flatten()
            .collect())
    }
}

glib::wrapper! {
    pub struct DMWindow(ObjectSubclass<imp::DMWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl DMWindow {
    pub fn new(
        app: &DMApplication,
        additional_search_paths: Vec<String>,
        ignore_default_paths: bool,
    ) -> Self {
        glib::Object::builder()
            .property("application", app)
            .property("additional_search_paths", additional_search_paths)
            .property("ignore_default_paths", ignore_default_paths)
            .build()
    }

    fn entries(&self) -> ListStore {
        self.imp()
            .entries
            .borrow()
            .clone()
            .expect("Entries not set")
    }
}
