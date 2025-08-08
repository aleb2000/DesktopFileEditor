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

use gtk::{gdk::{Key, ModifierType}, prelude::EntryExt};

/// This function is used to control the Escape key behavior in key pressed callbacks,
/// when the escape is pressed with popup completion the completion is disabled
/// and the event is consumed so that dialogs are not closed
///
/// Returns a boolean indicating weather the event should be consumed or not
#[allow(deprecated)]
pub fn entry_popup_completion_handle_escape_key_pressed(entry: &gtk::Entry, key: Key, modifier: ModifierType) -> bool {
    if modifier.is_empty() && key == Key::Escape {
        // escape pressed
        let completion = entry.completion().unwrap();
        if completion.is_popup_completion() {
            completion.set_popup_completion(false);
            // Consume key event
            true
        } else {
            false
        }
    } else {
        false
    }

}

macro_rules! connect_self_fn {
    ($self:ident.$fn:ident($arg:ident)) => {
        clone!(
            #[weak(rename_to=this)]
            $self,
            move |$arg| {this.$fn($arg)}
        )
    };

    ($self:ident.$fn:ident($arg:ident) -> $default_ret:expr) => {
        clone!(
            #[weak(rename_to=this)]
            $self,
            #[upgrade_or]
            $default_ret,
            move |$arg| {this.$fn($arg)}
        )
    };

    ($self:ident.$fn:ident($arg0:ident, $($args:ident),+)) => {
        clone!(
            #[weak(rename_to=this)]
            $self,
            move |$arg0 $(,$args)+| {this.$fn($arg0 $(,$args)+)}
        )
    };

    ($self:ident.$fn:ident($arg0:ident, $($args:ident),+) $(-> $default_ret:expr)?) => {
        clone!(
            #[weak(rename_to=this)]
            $self,
            #[upgrade_or]
            $($default_ret)?,
            move |$arg0 $(,$args)+| {this.$fn($arg0 $(,$args)+)}
        )
    };
}

pub(crate) use connect_self_fn;
