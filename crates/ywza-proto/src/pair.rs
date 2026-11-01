//! Pairing with a 6-digit code: SPAKE2 over a stream, key confirmation, then a MACed `PairInfo` swap.

use data_encoding::HEXLOWER;
use hmac::{Hmac, Mac};
use iroh::{EndpointAddr, EndpointId, SecretKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use crate::{read_frame, write_frame, Error};

/// ALPN of the pairing rendezvous endpoint.
pub const PAIR_ALPN: &[u8] = b"ywza/pair/1";

const CLIENT: &[u8] = b"ywza-client";
const AGENT: &[u8] = b"ywza-agent";

/// What each side learns about the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairInfo {
    pub node_id: EndpointId,
    pub name: String,
    pub addr: Option<EndpointAddr>,
}

#[derive(Serialize, Deserialize)]
struct Signed {
    json: String,
    mac: String,
}

/// Six random digits, zero padded.
pub fn new_code() -> String {
    format!("{:06}", Uuid::new_v4().as_u128() % 1_000_000)
}

/// Secret key of the rendezvous endpoint. Only the first 3 digits go in, so finding the
/// rendezvous does not reveal the full code.
pub fn rendezvous_key(code: &str) -> SecretKey {
    let head = code.get(..3).unwrap_or(code);
    let hash = Sha256::digest(format!("ywza-pair-v1/{head}"));
    SecretKey::from_bytes(&hash.into())
}

/// Agent end of the pairing handshake (SPAKE2 side B).
pub async fn agent_side<S, R>(
    send: &mut S,
    recv: &mut R,
    code: &str,
    me: PairInfo,
) -> Result<PairInfo, Error>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    run(send, recv, code, me, false).await
}

/// Client end of the pairing handshake (SPAKE2 side A). Writes first.
pub async fn client_side<S, R>(
    send: &mut S,
    recv: &mut R,
    code: &str,
    me: PairInfo,
) -> Result<PairInfo, Error>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    run(send, recv, code, me, true).await
}

async fn run<S, R>(
    send: &mut S,
    recv: &mut R,
    code: &str,
    me: PairInfo,
    client: bool,
) -> Result<PairInfo, Error>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let pw = Password::new(code.as_bytes());
    let (id_a, id_b) = (Identity::new(CLIENT), Identity::new(AGENT));
    let (state, out) = if client {
        Spake2::<Ed25519Group>::start_a(&pw, &id_a, &id_b)
    } else {
        Spake2::<Ed25519Group>::start_b(&pw, &id_a, &id_b)
    };
    let theirs: String = exchange(send, recv, &HEXLOWER.encode(&out), client).await?;
    let theirs = HEXLOWER
        .decode(theirs.as_bytes())
        .map_err(|e| Error::Pair(e.to_string()))?;
    let key = state
        .finish(&theirs)
        .map_err(|e| Error::Pair(e.to_string()))?;

    let (mine, other) = if client {
        (CLIENT, AGENT)
    } else {
        (AGENT, CLIENT)
    };
    let got: String = exchange(send, recv, &mac_hex(&key, mine), client).await?;
    check(&key, other, &got)?;

    let json = serde_json::to_string(&me)?;
    let out = Signed {
        mac: mac_hex(&key, json.as_bytes()),
        json,
    };
    let got: Signed = exchange(send, recv, &out, client).await?;
    check(&key, got.json.as_bytes(), &got.mac)?;
    Ok(serde_json::from_str(&got.json)?)
}

/// The client writes first because QUIC only announces a stream once data is written to it.
async fn exchange<S, R, T>(send: &mut S, recv: &mut R, out: &T, first: bool) -> Result<T, Error>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
    T: Serialize + DeserializeOwned,
{
    if first {
        write_frame(send, out).await?;
        read_frame(recv).await
    } else {
        let got = read_frame(recv).await?;
        write_frame(send, out).await?;
        Ok(got)
    }
}

fn new_mac(key: &[u8], data: &[u8]) -> Hmac<Sha256> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("hmac takes any key length");
    m.update(data);
    m
}

fn mac_hex(key: &[u8], data: &[u8]) -> String {
    HEXLOWER.encode(&new_mac(key, data).finalize().into_bytes())
}

fn check(key: &[u8], data: &[u8], hex: &str) -> Result<(), Error> {
    let tag = HEXLOWER.decode(hex.as_bytes()).map_err(|_| Error::BadCode)?;
    new_mac(key, data)
        .verify_slice(&tag)
        .map_err(|_| Error::BadCode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, addr: bool) -> PairInfo {
        let id = SecretKey::generate().public();
        PairInfo {
            node_id: id,
            name: name.into(),
            addr: addr.then(|| {
                EndpointAddr::new(id).with_ip_addr("10.0.0.2:4433".parse().unwrap())
            }),
        }
    }

    async fn pair(
        client_code: &str,
        agent_code: &str,
        c: PairInfo,
        a: PairInfo,
    ) -> (Result<PairInfo, Error>, Result<PairInfo, Error>) {
        let (mut c_send, mut a_recv) = tokio::io::duplex(4096);
        let (mut a_send, mut c_recv) = tokio::io::duplex(4096);
        tokio::join!(
            client_side(&mut c_send, &mut c_recv, client_code, c),
            agent_side(&mut a_send, &mut a_recv, agent_code, a),
        )
    }

    #[test]
    fn code_is_six_digits() {
        for _ in 0..50 {
            let c = new_code();
            assert_eq!(c.len(), 6);
            assert!(c.bytes().all(|b| b.is_ascii_digit()));
        }
    }

    #[test]
    fn rendezvous_key_uses_only_the_first_three_digits() {
        let a = rendezvous_key("123456").public();
        assert_eq!(a, rendezvous_key("123456").public());
        assert_eq!(a, rendezvous_key("123999").public());
        assert_ne!(a, rendezvous_key("124456").public());
    }

    #[tokio::test]
    async fn same_code_swaps_infos() {
        let (c, a) = (info("phone", false), info("pc", true));
        let (from_agent, from_client) = pair("123456", "123456", c.clone(), a.clone()).await;
        assert_eq!(from_agent.unwrap(), a);
        assert_eq!(from_client.unwrap(), c);
    }

    #[tokio::test]
    async fn wrong_code_is_bad_code_on_both_sides() {
        let (c, a) = (info("phone", false), info("pc", true));
        let (client, agent) = pair("123456", "654321", c, a).await;
        assert!(matches!(client, Err(Error::BadCode)));
        assert!(matches!(agent, Err(Error::BadCode)));
    }
}
