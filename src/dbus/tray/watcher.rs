//! `org.kde.StatusNotifierWatcher`: our implementation (when the session lacks one) and a proxy.

use std::sync::{Arc, Mutex};

use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, interface, proxy};

use super::{WATCHER, WATCHER_PATH};

#[proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
pub(super) trait Watcher {
    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;
}

struct WatcherService {
    items: Arc<Mutex<Vec<String>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl WatcherService {
    async fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        // Items may pass just an object path; then the sender is the bus name.
        let service = if service.starts_with('/') {
            match header.sender() {
                Some(sender) => format!("{sender}{service}"),
                None => return,
            }
        } else {
            service
        };
        let added = {
            let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
            if items.contains(&service) {
                false
            } else {
                items.push(service.clone());
                true
            }
        };
        if added {
            let _ = Self::status_notifier_item_registered(&emitter, &service).await;
        }
    }

    async fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Serves the watcher unless another process does. Returns our item list when we serve it.
pub(super) async fn serve_if_missing(
    connection: &Connection,
) -> zbus::Result<Option<Arc<Mutex<Vec<String>>>>> {
    let items = Arc::new(Mutex::new(Vec::new()));
    connection.object_server().at(WATCHER_PATH, WatcherService { items: items.clone() }).await?;
    let flags = RequestNameFlags::DO_NOT_QUEUE | RequestNameFlags::ALLOW_REPLACEMENT;
    match connection.request_name_with_flags(WATCHER, flags).await {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(Some(items)),
        // Another watcher (a different shell or panel) is running: share it.
        Ok(_) | Err(zbus::Error::NameTaken) => {
            connection.object_server().remove::<WatcherService, _>(WATCHER_PATH).await?;
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
