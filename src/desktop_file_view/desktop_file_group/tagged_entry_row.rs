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

use adw::subclass::prelude::ObjectSubclassExt;

use adw::prelude::*;
use gtk::glib::property::PropertySet;
use gtk::glib::subclass::types::ObjectSubclassIsExt;
use gtk::glib::{self, clone, closure_local};
use gtk::graphene::Point;
use tag::Tag;

pub const TAG_SPACING: i32 = 6;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use adw::subclass::prelude::*;
    use adw::{
        prelude::*, CallbackAnimationTarget, PropertyAnimationTarget, SpringAnimation,
        SpringParams, TimedAnimation, WrapBox,
    };
    use gtk::gdk::BUTTON_PRIMARY;
    use gtk::glib::subclass::Signal;
    use gtk::glib::{self, clone, Properties};
    use gtk::glib::{closure, closure_local, Object};
    use gtk::graphene::Point;
    use gtk::GestureDrag;

    use crate::desktop_file_view::languages::LANGUAGES_LOCALE_MAP;

    use super::tag::Tag;
    use super::{DragSide, TAG_SPACING};

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::TaggedEntryRow)]
    pub struct TaggedEntryRow {
        #[property(get, set)]
        pub localizable: Cell<bool>,

        #[property(get, set)]
        pub locale: RefCell<Option<String>>,

        pub tags_box: RefCell<WrapBox>,
        pub add_button: RefCell<Tag>,
        pub suffixes: RefCell<gtk::Box>,
        pub locale_emblem: RefCell<gtk::Label>,

        pub adding_tags: Cell<bool>,
        pub enter_anim: RefCell<Option<TimedAnimation>>,
        pub leave_anim: RefCell<Option<TimedAnimation>>,

        pub drag_gesture: RefCell<GestureDrag>,
        pub reordered_tag: RefCell<Option<Tag>>,
        pub reordered_tag_begin_index: Cell<Option<usize>>,
        pub reorder_anim: RefCell<Option<(TimedAnimation, TimedAnimation)>>,
        pub drag_bounceback_anim: RefCell<Option<(SpringAnimation, SpringAnimation)>>,
        moveaside_tags_info: RefCell<HashMap<Tag, MoveasideAnimationInfo>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TaggedEntryRow {
        const NAME: &'static str = "TaggedEntryRow";
        type Type = super::TaggedEntryRow;
        type ParentType = adw::PreferencesRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for TaggedEntryRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj().clone();
            obj.add_css_class("taggedentryrow");
            obj.set_vexpand(true);
            obj.set_hexpand(true);

            let container = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let suffixes = self.suffixes.borrow().clone();
            suffixes.set_orientation(gtk::Orientation::Vertical);
            suffixes.set_spacing(6);
            suffixes.set_visible(false);

            let main_content = gtk::Box::new(gtk::Orientation::Vertical, 6);

            let title_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let title_label = gtk::Label::builder()
                .halign(gtk::Align::Start)
                .justify(gtk::Justification::Left)
                .css_classes(["subtitle"])
                .build();
            obj.bind_property("title", &title_label, "label").build();

            let locale_emblem = self.locale_emblem.borrow().clone();
            locale_emblem.set_css_classes(&["locale-emblem", "caption"]);

            obj.property_expression_weak("locale")
                .chain_closure::<bool>(closure!(|_: Option<Object>, locale: Option<String>| {
                    locale.is_some()
                }))
                .bind(&locale_emblem, "visible", gtk::Widget::NONE);

            obj.bind_property("locale", &locale_emblem, "label").build();

            obj.property_expression_weak("locale")
                .chain_closure::<String>(closure!(|_: Option<Object>, locale: Option<String>| {
                    LANGUAGES_LOCALE_MAP
                        .get(&locale.unwrap_or_default()[..])
                        .unwrap_or(&"Locale")
                        .to_string()
                }))
                .bind(&locale_emblem, "tooltip_text", gtk::Widget::NONE);

            title_box.append(&title_label);
            title_box.append(&locale_emblem);

            let tags_box = self.tags_box.borrow().clone();
            tags_box.set_orientation(gtk::Orientation::Horizontal);
            tags_box.set_child_spacing(TAG_SPACING);
            tags_box.set_line_spacing(TAG_SPACING);
            tags_box.set_hexpand(true);
            tags_box.set_vexpand(true);
            tags_box.set_css_classes(&["tags"]);

            main_content.append(&title_box);
            main_content.append(&tags_box);
            container.append(&main_content);
            container.append(&suffixes);
            obj.set_child(Some(&container));

            let add_button = self.add_button.borrow().clone();
            add_button.set_icon_name("list-add-symbolic");
            add_button.set_button_tooltip(Some("Add"));
            add_button.set_editable(false);
            add_button.set_label_visible(false);
            tags_box.append(&add_button);

            add_button.connect_closure(
                "clicked",
                true,
                closure_local!(
                    #[weak]
                    obj,
                    move |_add_tag: Tag| {
                        obj.add_tag_and_edit();
                    }
                ),
            );

            // Init enter and leave animations
            let enter_anim = TimedAnimation::builder()
                .widget(&obj)
                .value_from(0.0)
                .value_to(1.0)
                .duration(300)
                .easing(adw::Easing::EaseOutQuart)
                // Initialize wit dummy target to suprress errors
                .target(&CallbackAnimationTarget::new(|_| {}))
                .build();
            self.enter_anim.replace(Some(enter_anim));

            let leave_anim = TimedAnimation::builder()
                .widget(&obj)
                .value_from(1.0)
                .value_to(0.0)
                .duration(300)
                .easing(adw::Easing::EaseInCubic)
                // Initialize wit dummy target to suprress errors
                .target(&CallbackAnimationTarget::new(|_| {}))
                .build();
            self.leave_anim.replace(Some(leave_anim));

            self.init_tag_reordering();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("changed").build()])
        }
    }

    impl WidgetImpl for TaggedEntryRow {}
    impl PreferencesRowImpl for TaggedEntryRow {}
    impl ListBoxRowImpl for TaggedEntryRow {}

    impl TaggedEntryRow {
        fn init_tag_reordering(&self) {
            let obj = self.obj().clone();

            let reorder_anim_new = || {
                TimedAnimation::builder()
                    .widget(&obj)
                    .duration(500)
                    .value_from(0.0)
                    .value_to(0.0)
                    .easing(adw::Easing::EaseOutBack)
                    // Initialize wit dummy target to suprress errors
                    .target(&CallbackAnimationTarget::new(|_| {}))
                    .build()
            };

            let drag_bounceback_anim_new = || {
                SpringAnimation::builder()
                    .widget(&obj)
                    .value_from(0.0)
                    .value_to(0.0)
                    .spring_params(&SpringParams::new(0.35, 1.0, 200.0))
                    .epsilon(0.032)
                    // Initialize wit dummy target to suprress errors
                    .target(&CallbackAnimationTarget::new(|_| {}))
                    .build()
            };

            self.reorder_anim
                .replace(Some((reorder_anim_new(), reorder_anim_new())));

            self.drag_bounceback_anim.replace(Some((
                drag_bounceback_anim_new(),
                drag_bounceback_anim_new(),
            )));

            let drag = self.drag_gesture.borrow().clone();
            drag.set_button(BUTTON_PRIMARY);

            drag.connect_drag_begin(clone!(
                #[weak(rename_to=this)]
                self,
                move |_drag, x, y| {
                    let obj = this.obj();
                    let tag = obj.find_tag_at_pos(x, y).map(|(tag, _)| tag);

                    if let Some(tag) = &tag {
                        if let Some((anim_x, anim_y)) = this.reorder_anim.borrow().clone() {
                            anim_x.skip();
                            anim_y.skip();
                            let anim_target_x = PropertyAnimationTarget::new(tag, "translate_x");
                            let anim_target_y = PropertyAnimationTarget::new(tag, "translate_y");
                            anim_x.set_target(&anim_target_x);
                            anim_y.set_target(&anim_target_y);
                        }
                        let index = obj.find_tag_index(tag);
                        this.reordered_tag_begin_index.set(index);
                    }

                    this.reordered_tag.replace(tag);
                }
            ));

            drag.connect_drag_update(clone!(
                #[weak(rename_to=this)]
                self,
                move |drag, offset_x, offset_y| this.on_drag_update(drag, offset_x, offset_y)
            ));

            drag.connect_drag_end(clone!(
                #[weak(rename_to=this)]
                self,
                move |_drag, _x, _y| {
                    let reordered_tag = match this.reordered_tag.replace(None) {
                        Some(tag) => tag,
                        None => return,
                    };

                    // Restore button sensitivity
                    reordered_tag.set_button_sensitive(true);

                    // Show bounceback animation
                    let (bounceback_anim_x, bounceback_anim_y) =
                        match this.drag_bounceback_anim.borrow().clone() {
                            Some(anims) => anims,
                            None => return,
                        };

                    if let Some((reorder_anim_x, reorder_anim_y)) =
                        this.reorder_anim.borrow().clone()
                    {
                        reorder_anim_x.pause();
                        reorder_anim_y.pause();
                    }

                    let target_x = PropertyAnimationTarget::new(&reordered_tag, "translate_x");
                    let target_y = PropertyAnimationTarget::new(&reordered_tag, "translate_y");
                    bounceback_anim_x.set_target(&target_x);
                    bounceback_anim_y.set_target(&target_y);
                    bounceback_anim_x.set_value_from(reordered_tag.translate_x().into());
                    bounceback_anim_y.set_value_from(reordered_tag.translate_y().into());
                    bounceback_anim_x.reset();
                    bounceback_anim_y.reset();
                    bounceback_anim_x.play();
                    bounceback_anim_y.play();

                    this.moveaside_tags_info.borrow_mut().clear();

                    // Trigger changed signal when order is changed
                    if let Some(begin_index) = this.reordered_tag_begin_index.get() {
                        let obj = this.obj();
                        if let Some(end_index) = obj.find_tag_index(&reordered_tag) {
                            if begin_index != end_index {
                                obj.emit_by_name::<()>("changed", &[]);
                            }
                        }
                        this.reordered_tag_begin_index.set(None);
                    }
                }
            ));

            obj.add_controller(drag);
        }

        fn on_drag_update(&self, drag: &GestureDrag, offset_x: f64, offset_y: f64) {
            let (start_x, start_y) = match drag.start_point() {
                Some((start_x, start_y)) => (start_x, start_y),
                None => return,
            };

            let obj = self.obj().clone();
            let x = start_x + offset_x;
            let y = start_y + offset_y;

            let reordered_tag = match self.reordered_tag.borrow().clone() {
                Some(reordered_tag) => reordered_tag,
                _ => return,
            };

            // Make sure button is not clickable during drag
            let distance_travelled = (offset_x * offset_x + offset_y * offset_y).sqrt();
            if distance_travelled >= 10.0 {
                reordered_tag.set_button_sensitive(false);
            }

            let (trans_value_to_x, trans_value_to_y) = if adw::is_animations_enabled(&obj) {
                // Calculate pull animation based on cursor distance and angle
                let (center_x, center_y) = match obj.tag_center_pos(&reordered_tag) {
                    Some(center) => center,
                    None => return,
                };

                let (offset_from_center_x, offset_from_center_y) = (x - center_x, y - center_y);
                let distance_from_center = (offset_from_center_x * offset_from_center_x
                    + offset_from_center_y * offset_from_center_y)
                    .sqrt();
                let magnitude = TAG_SPACING as f64 * (distance_from_center / 100.0).clamp(0.0, 1.0);
                let angle = offset_from_center_y.atan2(offset_from_center_x);
                let trans_x = magnitude * angle.cos();
                let trans_y = magnitude * angle.sin();
                reordered_tag.set_translate_x(trans_x as f32);
                reordered_tag.set_translate_y(trans_y as f32);
                (trans_x, trans_y)
            } else {
                (0.0, 0.0)
            };
            if let Some((anim_x, anim_y)) = self.reorder_anim.borrow().clone() {
                anim_x.set_value_to(trans_value_to_x);
                anim_y.set_value_to(trans_value_to_y);
            }

            let (tag, side) = match obj.find_tag_at_pos(x, y) {
                Some((tag, side)) if tag != reordered_tag => (tag, side),
                _ => return,
            };

            let insert_after = match side {
                DragSide::Left => tag.prev_sibling().map(|tag| {
                    tag.downcast::<Tag>()
                        .expect("insert_after widget is not a Tag")
                }),
                DragSide::Right => Some(tag),
            };

            // Avoid moving tag to its own position
            if let (Some(insert_after), Some(next_sibling)) = (
                &insert_after,
                insert_after.as_ref().and_then(|tag| tag.next_sibling()),
            ) {
                let reordered_widget = reordered_tag.clone().upcast::<gtk::Widget>();
                if *insert_after == reordered_widget || next_sibling == reordered_widget {
                    return;
                }
            }

            // Find range of widgets involved in the reorder, begin inclusive, end
            // exlusive, excluding the reordered tag
            let tags_box = self.tags_box.borrow();
            let add_button = self.add_button.borrow().clone();
            let mut item = tags_box
                .first_child()
                .expect("No child in tags box")
                .downcast::<Tag>()
                .expect("Child is not a tag");
            let (begin, end) = loop {
                match (reordered_tag.clone(), insert_after.clone()) {
                    (reordered_tag, None) => break (item, Some(reordered_tag)),
                    (reordered_tag, Some(insert_after)) if item == reordered_tag => {
                        break (
                            reordered_tag
                                .next_sibling()
                                .expect("next sibling does not exist")
                                .downcast::<super::Tag>()
                                .expect("next sibling is not a Tag"),
                            insert_after.next_sibling().and_then(|tag| {
                                let tag = tag
                                    .downcast::<super::Tag>()
                                    .expect("next sibling is not a Tag");
                                if tag == add_button {
                                    None
                                } else {
                                    Some(tag)
                                }
                            }),
                        )
                    }
                    (reordered_tag, Some(insert_after)) if item == insert_after => {
                        break (
                            insert_after
                                .next_sibling()
                                .unwrap()
                                .downcast::<super::Tag>()
                                .expect("next sibling is not a Tag"),
                            Some(reordered_tag),
                        )
                    }
                    _ => (),
                };
                item = item
                    .next_sibling()
                    .expect("Next sibling does not exist")
                    .downcast::<Tag>()
                    .expect("Sibling is not a Tag");
            };

            let reorder_range = obj.tag_range(&begin, end.as_ref());
            let mut moveaside_tags_info = self.moveaside_tags_info.borrow_mut();
            moveaside_tags_info.retain(|tag, _| reorder_range.contains(tag));
            for tag in reorder_range {
                let info = MoveasideAnimationInfo::new(
                    &tag,
                    tag.compute_point(&obj, &Point::new(tag.translate_x(), tag.translate_y()))
                        .expect("Could not get coordinates for moveaside animation before"),
                );
                moveaside_tags_info
                    .entry(tag)
                    .and_modify(|old_info| {
                        old_info.anim_x.pause();
                        old_info.anim_y.pause();
                        *old_info = info.clone()
                    })
                    .or_insert(info);
            }

            let reordered_from_pos = reordered_tag
                .compute_point(
                    &obj,
                    &Point::new(reordered_tag.translate_x(), reordered_tag.translate_y()),
                )
                .expect("Could not get coordinates before");

            // Perform reorder
            tags_box.reorder_child_after(&reordered_tag, insert_after.as_ref());

            // Run reorder animation
            glib::idle_add_local_once(clone!(
                #[weak(rename_to=this)]
                self,
                #[weak]
                obj,
                #[weak]
                reordered_tag,
                move || {
                    if let Some((anim_x, anim_y)) = this.reorder_anim.borrow().clone() {
                        if let Some(reordered_from_pos) =
                            obj.compute_point(&reordered_tag, &reordered_from_pos)
                        {
                            anim_x.set_value_from(reordered_from_pos.x() as f64);
                            anim_y.set_value_from(reordered_from_pos.y() as f64);
                            anim_x.reset();
                            anim_y.reset();
                            anim_x.play();
                            anim_y.play();
                        }
                    };

                    for (tag, info) in this.moveaside_tags_info.borrow().iter() {
                        if let Some(from_pos) = obj.compute_point(tag, &info.value_from) {
                            info.anim_x.set_value_from(from_pos.x() as f64);
                            info.anim_y.set_value_from(from_pos.y() as f64);
                            info.anim_x.reset();
                            info.anim_y.reset();
                            info.anim_x.play();
                            info.anim_y.play();
                        }
                    }
                }
            ));
        }
    }

    #[derive(Clone)]
    struct MoveasideAnimationInfo {
        value_from: Point,
        anim_x: TimedAnimation,
        anim_y: TimedAnimation,
    }

    impl MoveasideAnimationInfo {
        fn new(tag: &Tag, value_from: Point) -> MoveasideAnimationInfo {
            let anim_target_x = PropertyAnimationTarget::new(tag, "translate_x");
            let anim_target_y = PropertyAnimationTarget::new(tag, "translate_y");
            let new_anim = |target| {
                TimedAnimation::builder()
                    .widget(tag)
                    .duration(500)
                    .value_from(0.0)
                    .value_to(0.0)
                    .easing(adw::Easing::EaseOutQuint)
                    // Initialize wit dummy target to suprress errors
                    .target(target)
                    .build()
            };
            Self {
                value_from,
                anim_x: new_anim(&anim_target_x),
                anim_y: new_anim(&anim_target_y),
            }
        }
    }
}

glib::wrapper! {
    pub struct TaggedEntryRow(ObjectSubclass<imp::TaggedEntryRow>)
        @extends adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl TaggedEntryRow {
    pub fn new(localizable: bool, locale: Option<String>) -> Self {
        glib::Object::builder()
            .property("localizable", localizable)
            .property("locale", locale)
            .build()
    }

    pub fn from_string_list(localizable: bool, locale: Option<String>, list: &str) -> Self {
        let entry = Self::new(localizable, locale);
        entry.push_string_list(list);
        entry
    }

    pub fn add_tag(&self, tag: &str) -> Tag {
        let imp = self.imp();
        let tag = Tag::new(tag);
        // tag.set_icon_name("list-remove-symbolic");
        tag.set_icon_name("window-close-symbolic");
        tag.set_button_tooltip(Some("Remove"));

        let sibling = imp.add_button.borrow().clone().prev_sibling();
        let tags_box = imp.tags_box.borrow().clone();
        tags_box.insert_child_after(&tag, sibling.as_ref());

        // Remove button
        tag.connect_closure(
            "clicked",
            true,
            closure_local!(
                #[weak(rename_to=this)]
                self,
                move |tag: Tag| {
                    this.remove_tag(&tag);
                    this.emit_by_name::<()>("changed", &[]);
                }
            ),
        );

        tag.connect_closure(
            "edit-end",
            true,
            closure_local!(
                #[weak(rename_to=this)]
                self,
                move |tag: Tag| {
                    if tag.label().is_empty() {
                        // Necessary to prevent crash during focus change event delivery
                        glib::idle_add_local_once(clone!(
                            #[weak]
                            this,
                            #[weak]
                            tag,
                            move || {
                                this.grab_focus();
                                this.remove_tag(&tag);
                            }
                        ));
                    }
                }
            ),
        );

        tag.connect_closure(
            "edit-applied",
            true,
            closure_local!(
                #[weak]
                imp,
                move |tag: Tag| {
                    let obj = imp.obj();
                    if imp.adding_tags.get() {
                        if !tag.label().is_empty() {
                            obj.add_tag_and_edit();
                        } else {
                            imp.adding_tags.set(false);
                        }
                    }

                    if !tag.label().is_empty() {
                        obj.emit_by_name::<()>("changed", &[]);
                    }
                }
            ),
        );

        tag.connect_closure(
            "edit-cancelled",
            true,
            closure_local!(
                #[weak]
                imp,
                move |_tag: Tag| {
                    imp.adding_tags.set(false);
                }
            ),
        );

        if let Some(animation) = imp.enter_anim.borrow().clone() {
            animation.skip();
            let target = adw::PropertyAnimationTarget::new(&tag, "scale");
            animation.set_target(&target);
            animation.reset();
            animation.play();
        }
        tag
    }

    fn add_tag_and_edit(&self) {
        let tag = self.add_tag("");
        tag.set_edit_mode(true);
        self.imp().adding_tags.set(true);
    }

    pub fn values(&self) -> Vec<String> {
        let mut res = Vec::new();

        let mut child = self
            .imp()
            .tags_box
            .borrow()
            .first_child()
            .expect("No child in tags box");
        let add_button = self.imp().add_button.borrow().clone();
        while child != add_button {
            let tag = child.downcast::<Tag>().expect("Child is not a Tag");
            if !tag.label().is_empty() {
                res.push(tag.label());
            }
            child = tag.next_sibling().expect("Next sibling does not exist");
        }

        res
    }

    pub fn values_as_string_list(&self) -> String {
        let mut s = self.values().join(";");
        s.push(';');
        s
    }

    pub fn add_suffix(&self, suffix: &impl IsA<gtk::Widget>) {
        let suffixes = self.imp().suffixes.borrow().clone();
        suffixes.append(suffix);
        suffixes.set_visible(true);
    }

    pub fn change_locale(&self, locale: Option<String>, localized_string_list: &str) {
        if !self.localizable() {
            return;
        }

        let freeze_guard = self.freeze_notify();
        let imp = self.imp();
        imp.locale.set(locale);
        self.notify_locale();

        self.clear();

        self.push_string_list(localized_string_list);
        drop(freeze_guard);
    }

    fn push_string_list(&self, list: &str) {
        for item in list.split(';') {
            let item = item.trim();
            if !item.is_empty() {
                self.add_tag(item);
            }
        }
    }

    pub fn clear(&self) {
        let tags_box = self.imp().tags_box.borrow();
        let mut child = tags_box.first_child().expect("No child in tags box");
        let add_button = self.imp().add_button.borrow().clone();
        let mut to_remove = Vec::new();
        while child != add_button {
            to_remove.push(child.clone());
            child = child.next_sibling().expect("Next sibling does not exist");
        }

        for widget in to_remove {
            tags_box.remove(&widget);
        }
    }

    pub fn remove_tag(&self, tag: &Tag) {
        let imp = self.imp();
        if let Some(anim) = imp.leave_anim.borrow().clone() {
            anim.skip();
            let target = adw::PropertyAnimationTarget::new(tag, "scale");
            anim.set_target(&target);
            anim.reset();
            anim.connect_done(clone!(
                #[weak]
                imp,
                #[weak]
                tag,
                move |_anim| {
                    imp.tags_box.borrow().remove(&tag);
                }
            ));
            anim.play();
        }
    }

    fn find_tag_index(&self, tag: &Tag) -> Option<usize> {
        let tags_box = self.imp().tags_box.borrow();
        let mut child = tags_box.first_child().expect("No child in tags box");
        let add_button = self.imp().add_button.borrow().clone();
        let mut i = 0;
        while child != add_button {
            if &child == tag {
                return Some(i);
            }
            child = child.next_sibling().expect("Next sibling does not exist");
            i += 1;
        }
        None
    }

    fn find_tag_at_pos(&self, x: f64, y: f64) -> Option<(Tag, DragSide)> {
        let x = x as f32;
        let y = y as f32;
        let tag_spacing = TAG_SPACING as f32;

        let tags_box = self.imp().tags_box.borrow();
        let mut child = tags_box.first_child().expect("No child in tags box");
        let add_button = self.imp().add_button.borrow().clone();
        while child != add_button {
            let mut bounds = child.compute_bounds(self)?;
            bounds = bounds.expand(&Point::new(
                bounds.x() - tag_spacing / 2.0,
                bounds.y() - tag_spacing / 2.0,
            ));
            bounds = bounds.expand(&Point::new(
                bounds.x() + bounds.width() + tag_spacing / 2.0,
                bounds.y() + bounds.height() + tag_spacing / 2.0,
            ));
            if bounds.contains_point(&Point::new(x, y)) {
                let side = if x <= bounds.x() + bounds.width() / 2.0 {
                    DragSide::Left
                } else {
                    DragSide::Right
                };

                return Some((child.downcast::<Tag>().expect("Child is not a Tag"), side));
            }
            child = child.next_sibling().expect("Next sibling does not exist");
        }

        None
    }

    fn tag_range(&self, begin: &Tag, end: Option<&Tag>) -> Vec<Tag> {
        let add_button = self.imp().add_button.borrow().clone();
        let mut item = begin.clone();

        let mut res = Vec::new();
        while item != add_button && end.map(|end| end != &item).unwrap_or(true) {
            res.push(item.clone().downcast::<Tag>().expect("Item is not a tag"));
            item = item
                .next_sibling()
                .expect("Next sibling does not exist")
                .downcast::<Tag>()
                .expect("Sibling is not a Tag");
        }
        res
    }

    fn tag_center_pos(&self, tag: &Tag) -> Option<(f64, f64)> {
        tag.compute_bounds(self).map(|bounds| {
            let center = bounds.center();
            (center.x() as f64, center.y() as f64)
        })
    }
}

enum DragSide {
    Left,
    Right,
}

impl Default for TaggedEntryRow {
    fn default() -> Self {
        Self::new(false, None)
    }
}

mod tag {
    use adw::subclass::prelude::ObjectSubclassIsExt;
    use gtk::{
        glib::{self, object::ObjectExt, Object},
        prelude::*,
    };

    mod imp {
        use std::{
            cell::{Cell, RefCell},
            sync::OnceLock,
        };

        use adw::prelude::*;
        use adw::subclass::prelude::*;
        use gtk::{
            gdk::{self, BUTTON_PRIMARY},
            glib::{self, clone, subclass::Signal, Propagation, Properties},
            graphene::Point,
            gsk::Transform,
            Button, EventControllerFocus, EventControllerKey, GestureClick, Label, Text,
        };

        #[derive(Default, Properties)]
        #[properties(wrapper_type = super::Tag)]
        pub struct Tag {
            #[property(get, set)]
            label: RefCell<String>,

            #[property(get, set)]
            icon_name: RefCell<String>,

            #[property(get, set = Tag::set_edit_mode)]
            edit_mode: Cell<bool>,

            #[property(get, set, nullable)]
            button_tooltip: RefCell<Option<String>>,

            #[property(get, set, construct, default = true)]
            button_sensitive: Cell<bool>,

            #[property(get, set, default = true)]
            label_visible: Cell<bool>,

            #[property(get, set, construct, default = true)]
            editable: Cell<bool>,

            #[property(get, set, construct, default = 1.0)]
            scale: Cell<f32>,

            #[property(get, set, construct, default = 0.0)]
            translate_x: Cell<f32>,

            #[property(get, set, construct, default = 0.0)]
            translate_y: Cell<f32>,

            container: RefCell<gtk::Box>,
            pub text_stack: RefCell<adw::ViewStack>,
            pub text: RefCell<Text>,
            pub button: RefCell<Button>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for Tag {
            const NAME: &'static str = "Tag";
            type Type = super::Tag;
            type ParentType = gtk::Widget;

            fn class_init(klass: &mut Self::Class) {
                klass.set_css_name("tag");
            }
        }

        #[glib::derived_properties]
        impl ObjectImpl for Tag {
            fn constructed(&self) {
                self.parent_constructed();
                let obj = self.obj().clone();
                //obj.set_layout_manager(Some(BinLayout::new()));
                obj.set_halign(gtk::Align::Start);
                obj.set_valign(gtk::Align::Center);
                obj.set_vexpand(true);

                let container = self.container.borrow().clone();
                container.set_orientation(gtk::Orientation::Horizontal);
                container.set_spacing(3);
                container.set_css_classes(&["container"]);

                let text_stack = adw::ViewStack::new();
                let label = Label::new(None);
                let text = self.text.borrow().clone();
                text.set_propagate_text_width(true);
                text.set_placeholder_text(Some("Item"));
                text_stack.add_named(&label, Some("label"));
                text_stack.add_named(&text, Some("text"));

                let button = Button::builder().css_classes(["circular", "flat"]).build();
                container.append(&text_stack);
                container.append(&button);
                container.set_parent(&obj);

                obj.bind_property("label", &label, "label").build();
                obj.bind_property("label", &text, "text").build();
                obj.bind_property("label-visible", &text_stack, "visible")
                    .build();
                obj.bind_property("icon-name", &button, "icon-name").build();
                obj.bind_property("button-tooltip", &button, "tooltip-text")
                    .build();
                obj.bind_property("button-sensitive", &button, "sensitive")
                    .build();

                button.connect_clicked(clone!(
                    #[weak]
                    obj,
                    move |_| {
                        if obj.edit_mode() {
                            obj.apply_edit();
                        } else {
                            obj.emit_by_name::<()>("clicked", &[]);
                        }
                    }
                ));

                self.text_stack.replace(text_stack);
                self.button.replace(button);

                let queue_allocate = |tag: &super::Tag| {
                    tag.queue_allocate();
                };

                obj.connect_scale_notify(queue_allocate);
                obj.connect_translate_x_notify(queue_allocate);
                obj.connect_translate_y_notify(queue_allocate);

                self.init_text_editing();
            }

            fn dispose(&self) {
                self.container.borrow().unparent();
            }

            fn signals() -> &'static [Signal] {
                static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
                SIGNALS.get_or_init(|| {
                    vec![
                        Signal::builder("clicked").build(),
                        Signal::builder("edit-end").build(),
                        Signal::builder("edit-applied").build(),
                        Signal::builder("edit-cancelled").build(),
                    ]
                })
            }
        }

        impl WidgetImpl for Tag {
            fn measure(
                &self,
                orientation: gtk::Orientation,
                for_size: i32,
            ) -> (i32, i32, i32, i32) {
                self.parent_measure(orientation, for_size);
                let measurement = self.container.borrow().measure(orientation, for_size);
                measurement

                // let (minimum, natural, minimum_baseline, natural_baseline) = measurement;
                // let scale = self.scale.get();
                // (
                //     (minimum as f32 * scale) as i32,
                //     (natural as f32 * scale) as i32,
                //     (minimum_baseline as f32 * scale) as i32,
                //     (natural_baseline as f32 * scale) as i32,
                // )
            }

            fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
                let obj = self.obj();
                self.parent_size_allocate(width, height, baseline);
                let mut t = Transform::new();

                // Translate to make sure the transform is centered
                t = t
                    .translate(&Point::new(width as f32 / 2.0, height as f32 / 2.0))
                    .scale(obj.scale(), obj.scale())
                    .translate(&Point::new(obj.translate_x(), obj.translate_y()))
                    .translate(&Point::new(-width as f32 / 2.0, -height as f32 / 2.0));

                self.container
                    .borrow()
                    .allocate(width, height, baseline, Some(t));
            }
        }

        impl Tag {
            fn init_text_editing(&self) {
                let obj = self.obj();
                let text = self.text.borrow();

                let gesture = GestureClick::builder().button(BUTTON_PRIMARY).build();
                gesture.connect_released(clone!(
                    #[weak]
                    obj,
                    move |_gesture, n_press, _x, _y| {
                        if n_press == 2 {
                            obj.set_edit_mode(true);
                        }
                    }
                ));
                obj.add_controller(gesture);

                text.connect_activate(clone!(
                    #[weak]
                    obj,
                    move |_text| {
                        obj.apply_edit();
                    }
                ));

                let key_controller = EventControllerKey::new();
                key_controller.connect_key_pressed(clone!(
                    #[weak]
                    obj,
                    #[upgrade_or]
                    Propagation::Proceed,
                    move |_controller, key, _keycode, _modifier| {
                        if matches!(key, gdk::Key::Escape) {
                            obj.set_edit_mode(false);
                            obj.emit_by_name::<()>("edit-cancelled", &[]);
                            Propagation::Stop
                        } else {
                            Propagation::Proceed
                        }
                    }
                ));
                obj.add_controller(key_controller);

                let focus_controller = EventControllerFocus::new();
                focus_controller.connect_leave(clone!(
                    #[weak]
                    obj,
                    move |_controller| {
                        obj.set_edit_mode(false);
                        obj.emit_by_name::<()>("edit-cancelled", &[]);
                    }
                ));
                obj.add_controller(focus_controller);
            }

            fn set_edit_mode(&self, mut edit_mode: bool) {
                // Make sure tag is editable
                let obj = self.obj();
                edit_mode = edit_mode && obj.editable();

                let old_edit_mode = self.edit_mode.get();
                if old_edit_mode == edit_mode {
                    return;
                }

                self.edit_mode.set(edit_mode);
                let text = self.text.borrow();
                let text_stack = self.text_stack.borrow();
                let button = self.button.borrow();
                if edit_mode {
                    text.grab_focus();
                    text_stack.set_visible_child_name("text");
                    button.set_icon_name("adw-entry-apply-symbolic");
                    button.set_tooltip_text(Some("Apply"));
                } else {
                    text_stack.set_visible_child_name("label");
                    text.set_text(&obj.label());
                    button.set_icon_name(&obj.icon_name());
                    button.set_tooltip_text(obj.button_tooltip().as_deref());
                    obj.emit_by_name::<()>("edit-end", &[]);
                }
            }
        }
    }

    glib::wrapper! {
        pub struct Tag(ObjectSubclass<imp::Tag>)
            @extends gtk::Widget,
            @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl Tag {
        pub fn new(label: &str) -> Self {
            Object::builder().property("label", label).build()
        }

        fn apply_edit(&self) {
            let imp = self.imp();
            let new_label = imp.text.borrow().text();
            let trimmed_label = new_label.trim();
            if !trimmed_label.is_empty() {
                self.set_label(trimmed_label);
            }

            self.set_edit_mode(false);
            self.emit_by_name::<()>("edit-applied", &[]);
        }
    }

    impl Default for Tag {
        fn default() -> Self {
            Self::new("")
        }
    }
}
