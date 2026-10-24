//! Locking the session (`ext-session-lock-v1`) and checking passwords (PAM).

mod lock;
mod pam;

pub use lock::{LockEvent, lock_session, on_lock_event, session_locked, unlock_session};
pub use pam::{AuthError, authenticate, current_user, default_service};
