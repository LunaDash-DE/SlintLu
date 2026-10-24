//! Seat input: keyboard, pointer and cursor handling.

mod keyboard;
pub(crate) mod keys;
mod pointer;
mod seat;

pub(crate) use seat::SeatDevices;
