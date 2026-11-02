use crate::platform::linux::x::os;
use crate::{Error, Result};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::zbus::{self, proxy::CacheProperties};
use atspi::ObjectRefOwned;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// D-Bus calls have no timeout of their own and some apps never answer (seen: Java's
/// bridge, cinnamon-launcher), so each app/node step and each whole call is bounded.
const STEP: Duration = Duration::from_secs(2);
const WHOLE: Duration = Duration::from_secs(30);
const STAGE: Duration = Duration::from_secs(10);

/// A required step: its error or timeout ([`STAGE`]) names `what`.
pub(super) async fn stage<T, E: std::fmt::Display>(
    what: &str,
    f: impl Future<Output = std::result::Result<T, E>>,
) -> Result<T> {
    match tokio::time::timeout(STAGE, f).await {
        Ok(r) => r.map_err(|e| Error::Os(format!("{what}: {e}"))),
        Err(_) => Err(Error::Os(format!(
            "{what}: no answer in {}s",
            STAGE.as_secs()
        ))),
    }
}

/// `f`, or None if it errs or takes longer than [`STEP`].
pub(super) async fn step<T, E>(f: impl Future<Output = std::result::Result<T, E>>) -> Option<T> {
    tokio::time::timeout(STEP, f).await.ok()?.ok()
}

pub(super) fn block<T: Send>(f: impl Future<Output = Result<T>> + Send) -> Result<T> {
    std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(os)?;
            rt.block_on(async {
                tokio::time::timeout(WHOLE, f).await.unwrap_or_else(|_| {
                    Err(Error::Os("accessibility bus did not answer in time".into()))
                })
            })
        })
        .join()
        .unwrap_or_else(|_| Err(Error::Os("accessibility worker panicked".into())))
    })
}

/// A plain connection to the AT-SPI bus. Not `atspi::AccessibilityConnection`: with atspi's
/// default `p2p` feature its constructor queries every registered app, and one app that
/// never answers (seen: Java, cinnamon-launcher) blocks it forever.
pub(super) async fn connect() -> Result<zbus::Connection> {
    // Toolkits start exporting their tree only when this is on; best effort, once.
    static ENABLED: AtomicBool = AtomicBool::new(false);
    if !ENABLED.swap(true, Ordering::Relaxed) {
        let _ = step(atspi::connection::set_session_accessibility(true)).await;
    }
    let addr = match std::env::var("AT_SPI_BUS_ADDRESS") {
        Ok(a) => a,
        Err(_) => {
            let session = stage("session bus", zbus::Connection::session()).await?;
            let reply = stage(
                "AT-SPI bus address",
                session.call_method(
                    Some("org.a11y.Bus"),
                    "/org/a11y/bus",
                    Some("org.a11y.Bus"),
                    "GetAddress",
                    &(),
                ),
            )
            .await?;
            reply.body().deserialize::<String>().map_err(os)?
        }
    };
    let addr: zbus::Address = addr.parse().map_err(os)?;
    let builder = zbus::connection::Builder::address(addr).map_err(os)?;
    stage("AT-SPI bus", builder.build()).await
}

pub(super) async fn proxy(
    conn: &zbus::Connection,
    bus: &str,
    path: &str,
) -> Result<AccessibleProxy<'static>> {
    AccessibleProxy::builder(conn)
        .destination(bus.to_owned())
        .map_err(os)?
        .path(path.to_owned())
        .map_err(os)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(os)
}

pub(super) async fn proxy_of(
    conn: &zbus::Connection,
    r: &ObjectRefOwned,
) -> Result<AccessibleProxy<'static>> {
    match (r.is_null(), r.name_as_str()) {
        (false, Some(bus)) => proxy(conn, bus, r.path_as_str()).await,
        _ => Err(Error::Os("null accessible".into())),
    }
}
