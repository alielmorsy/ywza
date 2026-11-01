use std::{fmt, str::FromStr};

use data_encoding::BASE64URL_NOPAD;
use iroh::{
    dns::{DnsResolver, FallbackMode},
    endpoint::{presets, Connection, Incoming, RecvStream, SendStream},
    Endpoint, EndpointAddr, SecretKey,
};
use serde::{Deserialize, Serialize};

use crate::Error;

/// ALPN of the ywza control protocol.
pub const ALPN: &[u8] = b"ywza/1";

/// An agent's address as one copy-pasteable string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub addr: EndpointAddr,
}

/// base64url (no padding) of the ticket's JSON.
impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_vec(self).map_err(|_| fmt::Error)?;
        f.write_str(&BASE64URL_NOPAD.encode(&json))
    }
}

impl FromStr for Ticket {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let bytes = BASE64URL_NOPAD
            .decode(s.trim().as_bytes())
            .map_err(|e| Error::Ticket(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Ticket(e.to_string()))
    }
}

/// The control connection: its bi-directional stream carries framed [`crate::ClientMsg`] / [`crate::AgentMsg`].
pub struct Control {
    pub conn: Connection,
    pub send: SendStream,
    pub recv: RecvStream,
}

/// System DNS raced against public resolvers from the start, so the relay and address lookup
/// still resolve behind a router or VPN whose DNS times out or fails on them.
pub fn dns_resolver() -> DnsResolver {
    DnsResolver::builder()
        .with_system_defaults()
        .with_fallback_mode(FallbackMode::Eager)
        .build()
}

/// Binds an endpoint with the n0 defaults (public relays and address lookup) that accepts [`ALPN`].
pub async fn bind(secret_key: SecretKey) -> Result<Endpoint, Error> {
    Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .dns_resolver(dns_resolver())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(Error::transport)
}

/// Dials the ticket's agent and opens the control stream.
///
/// QUIC only announces a stream to the peer once data is written to it, so the
/// caller must send its first frame (`Hello`) before the agent's [`accept_control`] returns.
pub async fn connect(endpoint: &Endpoint, ticket: &Ticket) -> Result<Control, Error> {
    let conn = endpoint
        .connect(ticket.addr.clone(), ALPN)
        .await
        .map_err(Error::transport)?;
    let (send, recv) = conn.open_bi().await.map_err(Error::transport)?;
    Ok(Control { conn, send, recv })
}

/// Completes an incoming connection and accepts the client's control stream.
/// Check `control.conn.remote_id()` against the trusted peers afterwards.
pub async fn accept_control(incoming: Incoming) -> Result<Control, Error> {
    let conn = incoming.await.map_err(Error::transport)?;
    let (send, recv) = conn.accept_bi().await.map_err(Error::transport)?;
    Ok(Control { conn, send, recv })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_frame, write_frame, AgentMsg, ClientMsg};
    use iroh::RelayMode;

    fn addr() -> EndpointAddr {
        EndpointAddr::new(SecretKey::generate().public())
            .with_ip_addr("192.168.1.5:4433".parse().unwrap())
    }

    #[test]
    fn ticket_round_trips() {
        let t = Ticket { addr: addr() };
        let s = t.to_string();
        assert!(s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(s.parse::<Ticket>().unwrap(), t);
        assert!(matches!(
            "not a ticket!".parse::<Ticket>(),
            Err(Error::Ticket(_))
        ));
        assert!(matches!("AAAA".parse::<Ticket>(), Err(Error::Ticket(_))));
    }

    /// Two loopback endpoints, no relay, no address lookup: runs offline.
    async fn local_endpoint() -> Endpoint {
        Endpoint::builder(presets::Minimal)
            .relay_mode(RelayMode::Disabled)
            .bind_addr("127.0.0.1:0")
            .unwrap()
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn control_stream_over_loopback() {
        let server = local_endpoint().await;
        let client = local_endpoint().await;
        let ticket = Ticket {
            addr: EndpointAddr::new(server.id()).with_addrs(
                server
                    .bound_sockets()
                    .into_iter()
                    .map(iroh::TransportAddr::Ip),
            ),
        };
        let client_id = client.id();

        let serve = tokio::spawn({
            let server = server.clone();
            async move {
                let incoming = server.accept().await.unwrap();
                let mut c = accept_control(incoming).await.unwrap();
                assert_eq!(c.conn.remote_id(), client_id);
                let msg: ClientMsg = read_frame(&mut c.recv).await.unwrap();
                assert!(matches!(msg, ClientMsg::Hello { ref client_name, .. } if client_name == "t"));
                let reply = AgentMsg::Error {
                    message: "hi".into(),
                };
                write_frame(&mut c.send, &reply).await.unwrap();
                c.send.finish().unwrap();
                c.conn.closed().await;
            }
        });

        let mut c = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            connect(&client, &ticket),
        )
        .await
        .expect("connect timed out")
        .unwrap();
        let hello = ClientMsg::Hello {
            client_name: "t".into(),
            token: None,
        };
        write_frame(&mut c.send, &hello).await.unwrap();
        let reply: AgentMsg = read_frame(&mut c.recv).await.unwrap();
        assert_eq!(
            reply,
            AgentMsg::Error {
                message: "hi".into()
            }
        );

        c.conn.close(0u32.into(), b"bye");
        client.close().await;
        serve.await.unwrap();
        server.close().await;
    }
}
