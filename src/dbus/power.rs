//! Power actions and session lock signals through systemd-logind (system bus).

use futures_util::StreamExt;
use zbus::{Connection, OwnedObjectPath, proxy};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1Manager {
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn can_suspend(&self) -> zbus::Result<String>;
    fn can_hibernate(&self) -> zbus::Result<String>;
    fn get_session_by_pid(&self, pid: u32) -> zbus::Result<OwnedObjectPath>;
    fn get_session(&self, session_id: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

#[proxy(interface = "org.freedesktop.login1.Session", default_service = "org.freedesktop.login1")]
trait Login1Session {
    fn lock(&self) -> zbus::Result<()>;
    fn unlock(&self) -> zbus::Result<()>;
    fn terminate(&self) -> zbus::Result<()>;
    fn set_locked_hint(&self, locked: bool) -> zbus::Result<()>;
    fn set_brightness(&self, subsystem: &str, name: &str, brightness: u32) -> zbus::Result<()>;
    #[zbus(signal, name = "Lock")]
    fn lock_requested(&self) -> zbus::Result<()>;
    #[zbus(signal, name = "Unlock")]
    fn unlock_requested(&self) -> zbus::Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    PowerOff,
    Reboot,
    Suspend,
    Hibernate,
    /// End the current session.
    LogOut,
    /// Ask everything listening (including this shell) to lock the session.
    Lock,
}

/// Session events a shell reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// `loginctl lock-session` or an idle daemon asked to lock.
    Lock,
    Unlock,
    /// The machine is about to sleep (`true`) or has resumed (`false`).
    Sleep(bool),
}

#[derive(Clone)]
pub struct Power {
    connection: Connection,
}

impl Power {
    pub fn connect() -> zbus::Result<Self> {
        let connection = zbus::block_on(Connection::system())?;
        zbus::block_on(Login1ManagerProxy::new(&connection))?;
        Ok(Self { connection })
    }

    async fn session(&self) -> zbus::Result<Login1SessionProxy<'static>> {
        let manager = Login1ManagerProxy::new(&self.connection).await?;
        let path = match std::env::var("XDG_SESSION_ID") {
            Ok(id) => manager.get_session(&id).await?,
            Err(_) => manager.get_session_by_pid(std::process::id()).await?,
        };
        Login1SessionProxy::builder(&self.connection).path(path).build().await
    }

    pub fn perform(&self, action: PowerAction) -> zbus::Result<()> {
        zbus::block_on(async {
            let manager = Login1ManagerProxy::new(&self.connection).await?;
            match action {
                PowerAction::PowerOff => manager.power_off(true).await,
                PowerAction::Reboot => manager.reboot(true).await,
                PowerAction::Suspend => manager.suspend(true).await,
                PowerAction::Hibernate => manager.hibernate(true).await,
                PowerAction::LogOut => self.session().await?.terminate().await,
                PowerAction::Lock => self.session().await?.lock().await,
            }
        })
    }

    /// Whether suspend and hibernate are possible ("yes" or "challenge").
    pub fn capabilities(&self) -> (bool, bool) {
        zbus::block_on(async {
            let Ok(manager) = Login1ManagerProxy::new(&self.connection).await else {
                return (false, false);
            };
            let can =
                |answer: zbus::Result<String>| matches!(answer.as_deref(), Ok("yes" | "challenge"));
            (can(manager.can_suspend().await), can(manager.can_hibernate().await))
        })
    }

    /// Tells logind whether the session is locked (shown by `loginctl` and used by idle tools).
    pub fn set_locked_hint(&self, locked: bool) -> zbus::Result<()> {
        zbus::block_on(async { self.session().await?.set_locked_hint(locked).await })
    }

    /// Sets a backlight (`subsystem` "backlight") or LED through logind, which needs no root.
    pub fn set_brightness(&self, subsystem: &str, name: &str, value: u32) -> zbus::Result<()> {
        zbus::block_on(async { self.session().await?.set_brightness(subsystem, name, value).await })
    }

    /// Blocks, reporting lock / unlock requests and sleep transitions.
    pub fn watch(&self, mut on_event: impl FnMut(SessionEvent)) -> zbus::Result<()> {
        zbus::block_on(async {
            let manager = Login1ManagerProxy::new(&self.connection).await?;
            let session = self.session().await?;
            let mut sleep = manager.receive_prepare_for_sleep().await?;
            let mut lock = session.receive_lock_requested().await?;
            let mut unlock = session.receive_unlock_requested().await?;
            loop {
                futures_util::select! {
                    s = sleep.next() => match s.and_then(|s| s.args().ok().map(|a| a.start)) {
                        Some(start) => on_event(SessionEvent::Sleep(start)),
                        None => break,
                    },
                    l = lock.next() => match l { Some(_) => on_event(SessionEvent::Lock), None => break },
                    u = unlock.next() => match u { Some(_) => on_event(SessionEvent::Unlock), None => break },
                }
            }
            Err(zbus::Error::Failure("the system bus closed".into()))
        })
    }
}
