use std::{
    io,
    time::{Duration, Instant},
};

use shared::net::{
    ClientMessage, Config, Endpoint, ServerMessage, Transport, WsServer, dual::Dual,
    native::UdpTransport,
};

use shared::sim::TICK_RATE;

fn tick_dt() -> Duration {
    Duration::from_secs_f64(1.0 / TICK_RATE as f64)
}
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

    let tick = tick_dt();
    let mut next = Instant::now() + tick;

    loop {
        server.poll_network();

        let now = Instant::now();
        let mut ticks = 0;
        while now >= next && ticks < 5 {
            server.tick();
            next += tick;
            ticks += 1;
        }
        if now >= next + tick * 5 {
            next = now + tick;
        }

        let wait = next.saturating_duration_since(Instant::now());
        server
            .net
            .transport_mut()
            .wait(wait.min(Duration::from_millis(2)));
    }
}
