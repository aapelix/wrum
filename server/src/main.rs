use std::{
    io,
    time::{Duration, Instant},
};

use shared::net::{
    ClientMessage, Config, Endpoint, ServerMessage, Transport, WsServer, dual::Dual,
    native::UdpTransport,
};

pub const TICK_DT: Duration = Duration::from_millis(16);
pub const MAX_ACCUMULATOR: Duration = Duration::from_millis(250);

use crate::server::{Game, Server};

pub mod server;

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let udp_port = args.next().unwrap_or_else(|| "7777".into());
    let ws_port = args.next().unwrap_or_else(|| "7778".into());

    let transport = Dual::new(
        UdpTransport::bind(format!("0.0.0.0:{udp_port}"))?,
        WsServer::bind(format!("0.0.0.0:{ws_port}"))?,
    );

    println!("udp :{udp_port}  ws :{ws_port}");
    let endpoint = Endpoint::<_, ServerMessage, ClientMessage>::new(transport, Config::default());
    let mut server = Server {
        net: endpoint,
        game: Game::new(),
    };

    let mut last = Instant::now();
    let mut accumulator = Duration::ZERO;

    loop {
        let now = Instant::now();
        let dt = now - last;
        last = now;

        accumulator += dt;

        if accumulator > MAX_ACCUMULATOR {
            accumulator = MAX_ACCUMULATOR;
        }

        server.poll_network();

        while accumulator >= TICK_DT {
            accumulator -= TICK_DT;
            server.tick();
        }

        server.net.transport_mut().wait(TICK_DT / 4);
    }
}
