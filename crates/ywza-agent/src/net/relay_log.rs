use iroh::{Endpoint, Watcher};

/// Logs home relay connects, failures and changes at debug.
pub async fn watch(ep: Endpoint) {
    let mut status = ep.home_relay_status();
    while let Ok(relays) = status.updated().await {
        if relays.is_empty() {
            tracing::debug!("no home relay");
        }
        for r in &relays {
            tracing::debug!(
                relay = %r.url(),
                connected = r.is_connected(),
                last_error = ?r.last_error(),
                "home relay"
            );
        }
    }
}
