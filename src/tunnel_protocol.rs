use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::time::timeout;
use tokio_util::codec::Framed;
use tokio_util::codec::{AnyDelimiterCodec, FramedParts};
use uuid::Uuid;

/// TCP port used for control connections with the server.
pub const CONTROL_PORT: u16 = 4300;
pub const CONNECTION_TIMOUT: Duration = Duration::new(5, 0);

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ReqTunnel,
    RegProxy(Uuid),
}
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    NewTunnel(u16),
    Heartbeat,
    ReqProxy(Uuid),
}

pub struct Delimited<U>(Framed<U, AnyDelimiterCodec>);

impl<U: AsyncRead + AsyncWrite + Unpin> Delimited<U> {
    pub fn new(stream: U) -> Self {
        let codec = AnyDelimiterCodec::new(vec![0], vec![0]);
        Self(tokio_util::codec::Framed::new(stream, codec))
    }

    pub async fn send<T: Serialize>(&mut self, msg: T) -> Result<()> {
        self.0.send(serde_json::to_string(&msg)?).await?;
        Ok(())
    }

    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        if let Some(next_message) = self.0.next().await {
            let byte_message = next_message.context("frame error, invalid byte length")?;
            let serialized_obj =
                serde_json::from_slice(&byte_message).context("unable to parse message")?;
            Ok(serialized_obj)
        } else {
            Ok(None)
        }
    }

    pub async fn recv_timeout<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        timeout(CONNECTION_TIMOUT, self.recv())
            .await
            .context("timed out waiting for initial message")?
    }

    pub fn into_parts(self) -> FramedParts<U, AnyDelimiterCodec> {
        self.0.into_parts()
    }
}

pub async fn proxy<S1, S2>(stream1: S1, stream2: S2) -> io::Result<()>
where
    S1: AsyncRead + AsyncWrite + Unpin,
    S2: AsyncRead + AsyncWrite + Unpin,
{
    let (mut s1_read, mut s1_write) = io::split(stream1);
    let (mut s2_read, mut s2_write) = io::split(stream2);

    tokio::select! {
        res = io::copy(&mut s1_read, &mut s2_write) => res,
        res = io::copy(&mut s2_read, &mut s1_write) => res,
    }?;

    Ok(())
}
