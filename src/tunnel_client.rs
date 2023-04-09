use std::sync::Arc;

use crate::tunnel_protocol::{self, proxy, ClientMessage, Delimited, ServerMessage};
use anyhow::{bail, Context, Result};
use tokio::{net::TcpStream, time::timeout};
use tracing::{error, info, info_span, warn, Instrument};
use uuid::Uuid;

pub struct TcpClient {
    control_connection: Option<Delimited<TcpStream>>,
    remote_port: u16,
    local_port: u16,
}

impl TcpClient {
    /*
    initiates a long lived control connection to the server
    and completes the handshake with aztunnel server.
    */
    pub async fn new(localport: u16) -> Result<Self> {
        let tcp_stream = connect_with_timeout("127.0.0.1", tunnel_protocol::CONTROL_PORT).await?;
        let mut connection = Delimited::new(tcp_stream);

        // complete the authentication.

        /* initiate the tunnel handshake
         *
         * client -> ReqTunnel -> server
         * server -> NewTunnel(port) -> client.
         *
         */
        connection.send(ClientMessage::ReqTunnel).await?;

        let remote_port = match connection.recv_timeout().await? {
            Some(tunnel_protocol::ServerMessage::NewTunnel(remote_port)) => remote_port,
            Some(_) => bail!("unexpected initial non-hello message"),
            None => bail!("unexpected EOF"),
        };

        info!(remote_port, "connected to server");
        info!("listening at aztserver:{remote_port}");

        return Ok(TcpClient {
            control_connection: Some(connection),
            local_port: localport,
            remote_port: remote_port,
        });
    }

    pub async fn handle_server_request(mut self) -> Result<()> {
        let mut connection = self.control_connection.take().unwrap();
        let this = Arc::new(self);
        loop {
            match connection.recv().await? {
                Some(ServerMessage::NewTunnel(_)) => warn!("unexpected hello"),
                Some(ServerMessage::Heartbeat) => (),
                Some(ServerMessage::ReqProxy(id)) => {
                    let this = Arc::clone(&this);
                    tokio::spawn(
                        async move {
                            info!("new proxy connection");
                            match this.handle_connection(id).await {
                                Ok(_) => info!("connection exited"),
                                Err(err) => warn!(%err, "connection exited with error"),
                            }
                        }
                        .instrument(info_span!("proxy", %id)),
                    );
                }
                None => return Ok(()),
            }
        }
        return Ok(());
    }

    /**
     * Server requests client to initiate a connection to the server for the client(id)
     * that wants to send data to the service running on the local_port
     */
    async fn handle_connection(&self, id: Uuid) -> Result<()> {
        let proxy_tcp_stream =
            connect_with_timeout("127.0.0.1", tunnel_protocol::CONTROL_PORT).await?;

        let mut proxy_client = Delimited::new(proxy_tcp_stream);
        proxy_client.send(ClientMessage::RegProxy(id)).await?;

        let proxy_framed_parts = proxy_client.into_parts();

        let local_tcp_stream = connect_with_timeout("127.0.0.1", self.local_port).await?;
        proxy(proxy_framed_parts.io, local_tcp_stream).await?;
        Ok(())
    }
}

async fn connect_with_timeout(to: &str, port: u16) -> Result<TcpStream> {
    match timeout(
        tunnel_protocol::CONNECTION_TIMOUT,
        TcpStream::connect((to, port)),
    )
    .await
    {
        Ok(res) => res,
        Err(err) => Err(err.into()),
    }
    .with_context(|| format!("could not connect to {to}:{port}"))
}
