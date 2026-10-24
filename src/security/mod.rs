//! Locking the session (`ext-session-lock-v1`), checking passwords (PAM) and the polkit
//! authentication agent.

mod lock;
mod pam;
pub mod polkit;

pub use lock::{LockEvent, lock_session, on_lock_event, session_locked, unlock_session};
pub use pam::{AuthError, authenticate, current_user, default_service};
