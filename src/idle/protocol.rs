//! Protocol objects behind [`super::watch_idle`] and [`super::set_idle_inhibited`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::reexports::protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::{self, ExtIdleNotifierV1},
};
use smithay_client_toolkit::reexports::protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1::{self, ZwpIdleInhibitManagerV1},
    zwp_idle_inhibitor_v1::{self, ZwpIdleInhibitorV1},
};

use crate::core::state::State;

type Callback = Rc<dyn Fn(bool)>;

/// Idle globals and the active watches.
pub(crate) struct IdleState {
    notifier: Option<ExtIdleNotifierV1>,
    inhibit_manager: Option<ZwpIdleInhibitManagerV1>,
    seat: RefCell<Option<WlSeat>>,
    watches: RefCell<HashMap<u32, (ExtIdleNotificationV1, Callback)>>,
    next_id: Cell<u32>,
    inhibitor: RefCell<Option<ZwpIdleInhibitorV1>>,
}

/// User data of objects that send no events.
pub(crate) struct NoEvents;

/// User data of an idle notification: which watch it belongs to.
pub(crate) struct WatchId(u32);

impl IdleState {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        let notifier = globals.bind::<ExtIdleNotifierV1, State, NoEvents>(qh, 1..=1, NoEvents).ok();
        if notifier.is_none() {
            log::info!("ext-idle-notify-v1 unavailable: idle detection is disabled");
        }
        Self {
            notifier,
            inhibit_manager: globals
                .bind::<ZwpIdleInhibitManagerV1, State, NoEvents>(qh, 1..=1, NoEvents)
                .ok(),
            seat: RefCell::default(),
            watches: RefCell::default(),
            next_id: Cell::new(1),
            inhibitor: RefCell::default(),
        }
    }

    pub fn set_seat(&self, seat: &WlSeat) {
        self.seat.borrow_mut().get_or_insert_with(|| seat.clone());
    }

    pub fn add_watch(
        &self,
        qh: &QueueHandle<State>,
        timeout: Duration,
        callback: Callback,
    ) -> Option<u32> {
        let notifier = self.notifier.as_ref()?;
        let seat = self.seat.borrow().clone()?;
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
        let notification = notifier.get_idle_notification(millis, &seat, qh, WatchId(id));
        self.watches.borrow_mut().insert(id, (notification, callback));
        Some(id)
    }

    pub fn remove_watch(&self, id: u32) {
        if let Some((notification, _)) = self.watches.borrow_mut().remove(&id) {
            notification.destroy();
        }
    }

    fn callback(&self, id: u32) -> Option<Callback> {
        self.watches.borrow().get(&id).map(|(_, c)| c.clone())
    }

    pub fn inhibit(&self, surface: &WlSurface, qh: &QueueHandle<State>) -> bool {
        let Some(manager) = &self.inhibit_manager else { return false };
        self.uninhibit();
        *self.inhibitor.borrow_mut() = Some(manager.create_inhibitor(surface, qh, NoEvents));
        true
    }

    pub fn uninhibit(&self) {
        if let Some(inhibitor) = self.inhibitor.borrow_mut().take() {
            inhibitor.destroy();
        }
    }
}

impl Dispatch2<ExtIdleNotificationV1, State> for WatchId {
    fn event(
        &self,
        state: &mut State,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let idle = match event {
            ext_idle_notification_v1::Event::Idled => true,
            ext_idle_notification_v1::Event::Resumed => false,
            _ => return,
        };
        // Clone the callback out first: it may add or drop watches.
        if let Some(callback) = state.ctx.idle.callback(self.0) {
            callback(idle);
        }
    }
}

macro_rules! no_events {
    ($($interface:ty => $module:ident),* $(,)?) => {$(
        impl Dispatch2<$interface, State> for NoEvents {
            fn event(&self, _: &mut State, _: &$interface, _: $module::Event, _: &Connection, _: &QueueHandle<State>) {}
        }
    )*};
}

no_events!(
    ExtIdleNotifierV1 => ext_idle_notifier_v1,
    ZwpIdleInhibitManagerV1 => zwp_idle_inhibit_manager_v1,
    ZwpIdleInhibitorV1 => zwp_idle_inhibitor_v1,
);
