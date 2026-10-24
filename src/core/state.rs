//! The object Wayland events are dispatched to. The individual protocol handlers live next to
//! the feature they implement (`input`, `output`, `shell`, `core::compositor`).

use std::rc::{Rc, Weak};

use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::reexports::calloop::LoopHandle;
use smithay_client_toolkit::reexports::client::QueueHandle;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};

use super::context::Context;
use crate::input::SeatDevices;
use crate::window::WaylandWindow;

pub(crate) struct State {
    pub ctx: Rc<Context>,
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub loop_handle: LoopHandle<'static, State>,
    /// A second `wl_shm` binding, only used to receive format events.
    shm: Shm,
    pub devices: SeatDevices,
    pub keyboard_focus: Option<Weak<WaylandWindow>>,
    pub quit: bool,
}

impl State {
    pub fn new(
        ctx: Rc<Context>,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        loop_handle: LoopHandle<'static, State>,
    ) -> Self {
        Self {
            registry_state: RegistryState::new(globals),
            seat_state: SeatState::new(globals, qh),
            output_state: OutputState::new(globals, qh),
            shm: Shm::bind(globals, qh).expect("wl_shm was bound before"),
            devices: SeatDevices::new(globals, qh),
            ctx,
            loop_handle,
            keyboard_focus: None,
            quit: false,
        }
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_registry!(State);
delegate_dispatch2!(State);
