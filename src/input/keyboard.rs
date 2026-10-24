//! Keyboard focus and key events.

use std::rc::{Rc, Weak};

use slint::platform::WindowEvent;
use smithay_client_toolkit::reexports::client::protocol::{wl_keyboard, wl_surface};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};

use super::keys::key_text;
use crate::core::state::State;
use crate::window::WaylandWindow;

pub(crate) enum KeyKind {
    Press,
    Repeat,
    Release,
}

impl State {
    fn focused_window(&self) -> Option<Rc<WaylandWindow>> {
        self.keyboard_focus.as_ref().and_then(Weak::upgrade)
    }

    pub(crate) fn key_event(&mut self, event: KeyEvent, kind: KeyKind) {
        let Some(window) = self.focused_window() else {
            log::debug!("key event without a focused window: {:?}", event.keysym);
            return;
        };
        let Some(text) = key_text(&event) else { return };
        log::trace!("key {:?} -> {text:?}", event.keysym);
        window.dispatch(match kind {
            KeyKind::Press => WindowEvent::KeyPressed { text },
            KeyKind::Repeat => WindowEvent::KeyPressRepeated { text },
            KeyKind::Release => WindowEvent::KeyReleased { text },
        });
    }
}

impl KeyboardHandler for State {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        let window = self.ctx.find_window(surface);
        log::debug!(
            "keyboard focus enters {}",
            if window.is_some() { "a SlintLu window" } else { "an unknown surface" }
        );
        self.keyboard_focus = window.as_ref().map(Rc::downgrade);
        if let Some(window) = window {
            window.dispatch(WindowEvent::WindowActiveChanged(true));
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        if let Some(window) = self.keyboard_focus.take().and_then(|w| w.upgrade()) {
            window.dispatch(WindowEvent::WindowActiveChanged(false));
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, KeyKind::Press);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, KeyKind::Repeat);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, KeyKind::Release);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        // Slint derives the modifier state from the modifier key events themselves.
    }
}
