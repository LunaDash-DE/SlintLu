//! Copy and paste (`wl_data_device`) and the primary selection (middle-click paste), backing
//! Slint's clipboard so text fields work like in any other application, and a clipboard
//! history (`wlr-data-control`).

mod handlers;
mod history;
mod pipe;

pub(crate) use history::ClipboardHistory;
pub use history::{
    ClipboardEntry, clear_history, copy_from_history, history, history_available,
    on_history_changed, remove_from_history,
};

use std::cell::{Cell, RefCell};

use slint::platform::Clipboard;
use smithay_client_toolkit::data_device_manager::DataDeviceManagerState;
use smithay_client_toolkit::data_device_manager::data_device::DataDevice;
use smithay_client_toolkit::data_device_manager::data_source::CopyPasteSource;
use smithay_client_toolkit::primary_selection::PrimarySelectionManagerState;
use smithay_client_toolkit::primary_selection::device::PrimarySelectionDevice;
use smithay_client_toolkit::primary_selection::selection::PrimarySelectionSource;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};

use smithay_client_toolkit::reexports::protocols::wp::primary_selection::zv1::client::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;

use crate::core::state::State;

/// Mime types offered and accepted for text, best first.
pub(crate) const TEXT_MIME_TYPES: [&str; 5] =
    ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"];

/// Clipboard objects of the first seat and the text this client currently offers.
pub(crate) struct ClipboardState {
    manager: Option<DataDeviceManagerState>,
    primary_manager: Option<PrimarySelectionManagerState>,
    device: RefCell<Option<DataDevice>>,
    primary_device: RefCell<Option<PrimarySelectionDevice>>,
    source: RefCell<Option<(CopyPasteSource, String)>>,
    primary_source: RefCell<Option<(PrimarySelectionSource, String)>>,
    /// Serial of the latest key or button press, needed to set a selection.
    pub last_serial: Cell<u32>,
}

impl ClipboardState {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        Self {
            manager: DataDeviceManagerState::bind(globals, qh)
                .inspect_err(|e| log::info!("no clipboard support: {e}"))
                .ok(),
            primary_manager: PrimarySelectionManagerState::bind(globals, qh).ok(),
            device: RefCell::default(),
            primary_device: RefCell::default(),
            source: RefCell::default(),
            primary_source: RefCell::default(),
            last_serial: Cell::new(0),
        }
    }

    /// Creates the data devices for a newly announced seat (only the first seat is used).
    pub fn add_seat(&self, seat: &WlSeat, qh: &QueueHandle<State>) {
        if self.device.borrow().is_none()
            && let Some(manager) = &self.manager
        {
            *self.device.borrow_mut() = Some(manager.get_data_device(qh, seat));
        }
        if self.primary_device.borrow().is_none()
            && let Some(manager) = &self.primary_manager
        {
            *self.primary_device.borrow_mut() = Some(manager.get_selection_device(qh, seat));
        }
    }

    pub fn set_text(&self, text: &str, clipboard: Clipboard, qh: &QueueHandle<State>) {
        let serial = self.last_serial.get();
        match clipboard {
            Clipboard::DefaultClipboard => {
                let (Some(manager), Some(device)) = (&self.manager, &*self.device.borrow()) else {
                    return;
                };
                let source = manager.create_copy_paste_source(qh, TEXT_MIME_TYPES);
                source.set_selection(device, serial);
                *self.source.borrow_mut() = Some((source, text.to_owned()));
            }
            Clipboard::SelectionClipboard => {
                let (Some(manager), Some(device)) =
                    (&self.primary_manager, &*self.primary_device.borrow())
                else {
                    return;
                };
                let source = manager.create_selection_source(qh, TEXT_MIME_TYPES);
                source.set_selection(device, serial);
                *self.primary_source.borrow_mut() = Some((source, text.to_owned()));
            }
            _ => {}
        }
    }

    pub fn text(&self, clipboard: Clipboard, conn: &Connection) -> Option<String> {
        match clipboard {
            Clipboard::DefaultClipboard => {
                // Our own selection is answered without a round trip through the compositor.
                if let Some((_, text)) = &*self.source.borrow() {
                    return Some(text.clone());
                }
                let offer = self.device.borrow().as_ref()?.data().selection_offer()?;
                let mime = offer.with_mime_types(pick_mime)?;
                let pipe = offer.receive(mime).ok()?;
                let _ = conn.flush();
                pipe::read_to_string(pipe)
            }
            Clipboard::SelectionClipboard => {
                if let Some((_, text)) = &*self.primary_source.borrow() {
                    return Some(text.clone());
                }
                let offer = self.primary_device.borrow().as_ref()?.data().selection_offer()?;
                let mime = offer.with_mime_types(pick_mime)?;
                let pipe = offer.receive(mime).ok()?;
                let _ = conn.flush();
                pipe::read_to_string(pipe)
            }
            _ => None,
        }
    }
}

impl ClipboardState {
    fn offered_text(
        &self,
        source: &smithay_client_toolkit::reexports::client::protocol::wl_data_source::WlDataSource,
    ) -> Option<String> {
        self.source.borrow().as_ref().filter(|(s, _)| s.inner() == source).map(|(_, t)| t.clone())
    }

    fn cancel(
        &self,
        source: &smithay_client_toolkit::reexports::client::protocol::wl_data_source::WlDataSource,
    ) {
        let mut current = self.source.borrow_mut();
        if current.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            *current = None;
        }
    }

    fn offered_primary_text(&self, source: &ZwpPrimarySelectionSourceV1) -> Option<String> {
        self.primary_source
            .borrow()
            .as_ref()
            .filter(|(s, _)| s.inner() == source)
            .map(|(_, t)| t.clone())
    }

    fn cancel_primary(&self, source: &ZwpPrimarySelectionSourceV1) {
        let mut current = self.primary_source.borrow_mut();
        if current.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            *current = None;
        }
    }
}

fn pick_mime(offered: &[String]) -> Option<String> {
    TEXT_MIME_TYPES.iter().find(|m| offered.iter().any(|o| o == *m)).map(|m| m.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_utf8_text() {
        let offered =
            vec!["image/png".to_string(), "STRING".into(), "text/plain;charset=utf-8".into()];
        assert_eq!(pick_mime(&offered).as_deref(), Some("text/plain;charset=utf-8"));
        assert_eq!(pick_mime(&["image/png".to_string()]), None);
    }
}
