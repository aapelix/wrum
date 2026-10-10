use wtransport::{Endpoint, Identity, ServerConfig, endpoint::IncomingSession};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])?;
    let config = ServerConfig::builder()
        .with_bind_default(7778)
        .with_identity(identity)
        .build();

    let server = Endpoint::server(config)?;
    loop {
        let incoming_session = server.accept().await;
        tokio::spawn(handle_connection(incoming_session));
    }
}

async fn handle_connection(incoming_session: IncomingSession) -> anyhow::Result<()> {
    let req = incoming_session.await?;
    let conn = req.accept().await?;
    loop {
        tokio::select! {
            stream = conn.accept_uni() => {
                let mut stream = stream?;
                let _ = stream;
            }
            dgram = conn.receive_datagram() => {
                let dgram = dgram?;
                println!("recv (dgram): {dgram:#?}")
            }
        }
    }
}
