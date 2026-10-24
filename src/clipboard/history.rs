//! Clipboard history through `zwlr_data_control_v1`: every text that becomes the selection is
//! remembered (in memory only), and any entry can be put back on the clipboard.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::backend::ObjectData;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat;
use smithay_client_toolkit::reexports::client::{Connection, Proxy, QueueHandle};
use smithay_client_toolkit::reexports::protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::{self, ZwlrDataControlManagerV1},
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

use super::TEXT_MIME_TYPES;
use crate::core::context;
use crate::core::state::State;

/// Entries kept, newest first.
const MAX_ENTRIES: usize = 50;
/// Texts larger than this are not remembered.
const MAX_TEXT: usize = 1024 * 1024;
/// Password managers mark secrets with this type; they are never remembered.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";

/// One remembered clipboard text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: u64,
    pub text: String,
}

pub(crate) struct ClipboardHistory {
    manager: Option<ZwlrDataControlManagerV1>,
    device: RefCell<Option<ZwlrDataControlDeviceV1>>,
    entries: RefCell<Vec<ClipboardEntry>>,
    next_id: Cell<u64>,
    source: RefCell<Option<ZwlrDataControlSourceV1>>,
    listeners: RefCell<Vec<Rc<dyn Fn()>>>,
}

/// Mime types an offer announced.
#[derive(Default)]
pub(crate) struct OfferData {
    mimes: Mutex<Vec<String>>,
}

pub(crate) struct DeviceData;
pub(crate) struct ManagerData;

/// The text a source of ours serves.
pub(crate) struct SourceData {
    text: Arc<String>,
}

impl ClipboardHistory {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        let manager = globals
            .bind::<ZwlrDataControlManagerV1, State, ManagerData>(qh, 1..=2, ManagerData)
            .ok();
        if manager.is_none() {
            log::info!("wlr-data-control unavailable: no clipboard history");
        }
        Self {
            manager,
            device: RefCell::default(),
            entries: RefCell::default(),
            next_id: Cell::new(1),
            source: RefCell::default(),
            listeners: RefCell::default(),
        }
    }

    pub fn available(&self) -> bool {
        self.manager.is_some()
    }

    pub fn add_seat(&self, seat: &WlSeat, qh: &QueueHandle<State>) {
        if self.device.borrow().is_none()
            && let Some(manager) = &self.manager
        {
            *self.device.borrow_mut() = Some(manager.get_data_device(seat, qh, DeviceData));
        }
    }

    fn notify(&self) {
        let listeners = self.listeners.borrow().clone();
        for listener in listeners {
            listener();
        }
    }

    fn remember(&self, text: String) {
        if text.trim().is_empty() || text.len() > MAX_TEXT {
            return;
        }
        {
            let mut entries = self.entries.borrow_mut();
            if entries.first().is_some_and(|e| e.text == text) {
                return;
            }
            entries.retain(|e| e.text != text);
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            entries.insert(0, ClipboardEntry { id, text });
            entries.truncate(MAX_ENTRIES);
        }
        self.notify();
    }

    /// Reads the text of a new selection off the UI thread.
    fn receive(&self, offer: &ZwlrDataControlOfferV1, conn: &Connection) {
        let mimes = offer
            .data::<OfferData>()
            .map(|d| d.mimes.lock().unwrap_or_else(|e| e.into_inner()).clone());
        let mimes = mimes.unwrap_or_default();
        if mimes.iter().any(|m| m == SECRET_HINT) {
            return;
        }
        let Some(mime) = TEXT_MIME_TYPES.iter().find(|m| mimes.iter().any(|o| o == *m)) else {
            return;
        };
        let Ok((read, write)) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC) else {
            return;
        };
        offer.receive(mime.to_string(), write.as_fd());
        let _ = conn.flush();
        drop(write);
        let read: OwnedFd = read;
        std::thread::spawn(move || {
            let Some(text) = super::pipe::read_to_string(std::fs::File::from(read)) else { return };
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ctx) = context::current() {
                    ctx.clipboard_history.remember(text);
                }
            });
        });
    }

    fn copy(&self, text: String, qh: &QueueHandle<State>) {
        let (Some(manager), Some(device)) = (&self.manager, &*self.device.borrow()) else { return };
        let source = manager.create_data_source(qh, SourceData { text: Arc::new(text) });
        for mime in TEXT_MIME_TYPES {
            source.offer(mime.to_string());
        }
        device.set_selection(Some(&source));
        if let Some(old) = self.source.borrow_mut().replace(source) {
            old.destroy();
        }
    }
}

/// The remembered texts, newest first.
pub fn history() -> Vec<ClipboardEntry> {
    context::current().map(|ctx| ctx.clipboard_history.entries.borrow().clone()).unwrap_or_default()
}

/// Whether the compositor allows a clipboard history (`wlr-data-control`).
pub fn history_available() -> bool {
    context::current().is_some_and(|ctx| ctx.clipboard_history.available())
}

/// Calls `callback` after the history changed.
pub fn on_history_changed(callback: impl Fn() + 'static) {
    if let Some(ctx) = context::current() {
        ctx.clipboard_history.listeners.borrow_mut().push(Rc::new(callback));
    }
}

/// Puts an entry back on the clipboard.
pub fn copy_from_history(id: u64) {
    let Some(ctx) = context::current() else { return };
    let text =
        ctx.clipboard_history.entries.borrow().iter().find(|e| e.id == id).map(|e| e.text.clone());
    if let Some(text) = text {
        ctx.clipboard_history.copy(text, &ctx.qh);
        let _ = ctx.conn.flush();
    }
}

/// Forgets one entry.
pub fn remove_from_history(id: u64) {
    if let Some(ctx) = context::current() {
        ctx.clipboard_history.entries.borrow_mut().retain(|e| e.id != id);
        ctx.clipboard_history.notify();
    }
}

/// Forgets everything.
pub fn clear_history() {
    if let Some(ctx) = context::current() {
        ctx.clipboard_history.entries.borrow_mut().clear();
        ctx.clipboard_history.notify();
    }
}

impl Dispatch2<ZwlrDataControlManagerV1, State> for ManagerData {
    fn event(
        &self,
        _: &mut State,
        _: &ZwlrDataControlManagerV1,
        _: zwlr_data_control_manager_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch2<ZwlrDataControlDeviceV1, State> for DeviceData {
    fn event(
        &self,
        state: &mut State,
        device: &ZwlrDataControlDeviceV1,
        event: zwlr_data_control_device_v1::Event,
        conn: &Connection,
        _: &QueueHandle<State>,
    ) {
        use zwlr_data_control_device_v1::Event;
        match event {
            Event::Selection { id: Some(offer) } => {
                state.ctx.clipboard_history.receive(&offer, conn);
                offer.destroy();
            }
            Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
            Event::Finished => {
                device.destroy();
                state.ctx.clipboard_history.device.borrow_mut().take();
            }
            _ => {}
        }
    }

    fn event_created_child(opcode: u16, qh: &QueueHandle<State>) -> Arc<dyn ObjectData> {
        match opcode {
            zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => {
                qh.make_data::<ZwlrDataControlOfferV1, OfferData>(OfferData::default())
            }
            _ => panic!("unexpected child object for zwlr_data_control_device_v1 event {opcode}"),
        }
    }
}

impl Dispatch2<ZwlrDataControlOfferV1, State> for OfferData {
    fn event(
        &self,
        _: &mut State,
        _: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            self.mimes.lock().unwrap_or_else(|e| e.into_inner()).push(mime_type);
        }
    }
}

impl Dispatch2<ZwlrDataControlSourceV1, State> for SourceData {
    fn event(
        &self,
        state: &mut State,
        source: &ZwlrDataControlSourceV1,
        event: zwlr_data_control_source_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        match event {
            zwlr_data_control_source_v1::Event::Send { fd, .. } => {
                let text = self.text.clone();
                std::thread::spawn(move || {
                    let _ = std::fs::File::from(fd).write_all(text.as_bytes());
                });
            }
            zwlr_data_control_source_v1::Event::Cancelled => {
                source.destroy();
                let mut current = state.ctx.clipboard_history.source.borrow_mut();
                if current.as_ref() == Some(source) {
                    *current = None;
                }
            }
            _ => {}
        }
    }
}
