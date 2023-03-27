//! Create pipe-like communication channels using TURN servers (with mobility support)
//! One host can create such `socketpair`-like unreliable channel, then send credentials to second and third hosts to communicate.
//!
//! It creates two TURN allocations with Mobiolity connected to each other.
//!
//! Should be used from Tokio loop.
//!
//! Does not offer reliability, protection against eavesdropping or attacks, fragmentation; one write to the sink = one UDP packet.
//! 
//! Specifiers contain username and password in cleartext.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    task::Poll, io::Write,
};

use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::Bytes;
use flate2::{Compression, write::{ZlibEncoder, ZlibDecoder}};
use futures::{Sink, SinkExt, Stream, StreamExt};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tokio::{net::UdpSocket, select};
use turnclient::{
    ChannelUsage, MessageFromTurnServer, MessageToTurnServer, TurnClient, TurnClientBuilder, ExportedParameters,
};

#[derive(Serialize, Deserialize)]
struct Data {
    turn_server: SocketAddr,
    username: String,
    password: String,
    realm: String,
    nonce: String,
    mobility_ticket: Vec<u8>,
    counterpart: SocketAddr,
}

impl Data {
    pub fn new(turn_server: SocketAddr, username: String, password: String, state: ExportedParameters, counterpart: SocketAddr) -> Data {
        Data {
            turn_server,
            username,
            password,
            realm: state.realm,
            nonce: state.nonce,
            mobility_ticket: state.mobility_ticket,
            counterpart,
        }
    }
    pub fn serialize(&self) -> String {
        let q = bincode::serialize(self).unwrap();
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&q).unwrap();
        STANDARD.encode(e.finish().unwrap())
    }
    pub fn deserialize(x: &str) -> anyhow::Result<Data> {
        let z = STANDARD.decode(x)?;
        let mut d = ZlibDecoder::new(Vec::new());
        d.write_all(&z)?;
        let b = d.finish()?;
        Ok(bincode::deserialize(&b)?)
    }
}

/// Create a pair of allocations and serialize their parameters to string blobs
pub async fn tie(
    turn_server: SocketAddr,
    username: String,
    password: String,
) -> anyhow::Result<(String, String)> {
    let mut t1 = TurnClientBuilder::new(turn_server, username.clone(), password.clone());
    let mut t2 = TurnClientBuilder::new(turn_server, username.clone(), password.clone());
    t1.enable_mobility = true;
    t2.enable_mobility = true;

    let neutral_sockaddr = match turn_server {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let u1 = UdpSocket::bind(neutral_sockaddr).await?;
    let u2 = UdpSocket::bind(neutral_sockaddr).await?;
    u1.connect(turn_server).await?;
    u2.connect(turn_server).await?;

    let mut c1 = t1.build_and_send_request(u1);
    let mut c2 = t2.build_and_send_request(u2);

    //let (mut c1tx, mut c1rx) = c1.split();
    //let (mut c2tx, mut c2rx) = c2.split();

    let mut addr1 = None::<SocketAddr>;
    let mut addr2 = None::<SocketAddr>;
    let mut ready1 = false;
    let mut ready2 = false;

    let mut perm_requests_sent = false;

    loop {
        let (msg, first): (Option<Result<MessageFromTurnServer, _>>, bool) = select! {
            msg = c1.next() => {
                (msg, true)
            }
            msg = c2.next() => {
                (msg, false)
            }
        };
        let msg = msg.context(anyhow!("Sudden end of TURN client incoming messages"))??;
        match msg {
            MessageFromTurnServer::AllocationGranted {
                relay_address,
                mobility,
                ..
            } => {
                if !mobility {
                    anyhow::bail!("No RFC 8016 mobility received from TURN server");
                }
                if first {
                    addr1 = Some(relay_address);
                } else {
                    addr2 = Some(relay_address);
                }
            }
            MessageFromTurnServer::RedirectedToAlternateServer(alt) => {
                anyhow::bail!(
                    "We are being redirected to {alt}. This is not supported by turntie."
                );
            }
            MessageFromTurnServer::PermissionCreated(addr) => {
                if first && Some(addr) == addr2 {
                    ready1 = true;
                } else if !first && Some(addr) == addr1 {
                    ready2 = true;
                } else {
                    anyhow::bail!(
                        "Unexpected granted permission. Something is wrong with the code?"
                    );
                }
            }
            MessageFromTurnServer::PermissionNotCreated(_) => {
                anyhow::bail!("Failed to create permission on TURN server")
            }
            MessageFromTurnServer::Disconnected => anyhow::bail!("Disconnected from TURN server"),
            _ => (),
        }

        if addr1.is_some() && addr2.is_some() && !perm_requests_sent {
            perm_requests_sent = true;

            // deadlock risk?
            c1.send(MessageToTurnServer::AddPermission(
                addr2.unwrap(),
                ChannelUsage::JustPermission,
            ))
            .await?;
            c2.send(MessageToTurnServer::AddPermission(
                addr1.unwrap(),
                ChannelUsage::JustPermission,
            ))
            .await?;
        }

        if ready1 && ready2 {
            break;
        }
    }

    let params1 = c1.export_state();
    let params2 = c2.export_state();

    let spec1 = Data::new(turn_server, username.clone(), password.clone(), params1, addr2.unwrap());
    let spec2 = Data::new(turn_server, username, password, params2, addr1.unwrap());

    Ok((spec1.serialize(), spec2.serialize()))
}

/// Load one of string blobs created by [`tie`] and use it for communication.
/// You may want to use `Stream::split` for the resulting object to feed (receive) byte buffers to (from) it.
///
/// Withholding from polling the stream part of the object can eventually hinder sink part as well.
pub async fn connect(specifier: &str) -> anyhow::Result<TurnTie> {
    let data = Data::deserialize(specifier)?;

    let mut t = TurnClientBuilder::new(data.turn_server, data.username, data.password);
    t.enable_mobility = true;

    let neutral_sockaddr = match data.turn_server {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let u = UdpSocket::bind(neutral_sockaddr).await?;

    let params = ExportedParameters {
        realm: data.realm,
        nonce: data.nonce,
        mobility_ticket: data.mobility_ticket,
        permissions: vec![
            (data.counterpart, None)
        ]
    };

    let turnclient = t.restore_from_exported_parameters(u, &params)?;

    Ok(TurnTie {
        turnclient,
        counterpart: data.counterpart,
    })
}

#[pin_project]
pub struct TurnTie {
    #[pin]
    turnclient: TurnClient,
    counterpart: SocketAddr,
}

impl Stream for TurnTie {
    type Item = anyhow::Result<Bytes>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        'main_loop: loop {
            return match this.turnclient.as_mut().poll_next(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Some(Ok(msg))) => match msg {
                    MessageFromTurnServer::RecvFrom(fromaddr, buf) => {
                        if fromaddr == *this.counterpart {
                            return Poll::Ready(Some(Ok(buf.into())));
                        }
                        continue 'main_loop;
                    }
                    _ => continue 'main_loop,
                },
            };
        }
    }
}

impl Sink<Bytes> for TurnTie {
    type Error = anyhow::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.turnclient.poll_ready(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        let msg = MessageToTurnServer::SendTo(self.counterpart, item.into());
        let this = self.project();
        this.turnclient.start_send(msg)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.turnclient.poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.turnclient.poll_close(cx)
    }
}
