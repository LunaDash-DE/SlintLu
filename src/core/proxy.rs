//! Thread-safe handle used by Slint to wake up and stop the event loop.

use slint::platform::EventLoopProxy;
use smithay_client_toolkit::reexports::calloop::channel;

pub(crate) enum LoopMessage {
    Invoke(Box<dyn FnOnce() + Send>),
    Quit,
}

pub(crate) struct Proxy {
    pub sender: channel::Sender<LoopMessage>,
}

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.sender.send(LoopMessage::Quit).map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.sender
            .send(LoopMessage::Invoke(event))
            .map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
}
