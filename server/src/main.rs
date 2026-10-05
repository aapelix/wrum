use std::{io, time::Instant};

use shared::net::{
    ClientMessage, Config, Endpoint, Event, ServerMessage, Transport, WsServer, dual::Dual,
    native::UdpTransport,
};

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let udp_port = args.next().unwrap_or_else(|| "7777".into());
    let ws_port = args.next().unwrap_or_else(|| "7778".into());

    let transport = Dual::new(
        UdpTransport::bind(format!("0.0.0.0:{udp_port}"))?,
        WsServer::bind(format!("0.0.0.0:{ws_port}"))?,
    );

    println!("udp :{udp_port}  ws :{ws_port}");
    let mut server = Endpoint::<_, ServerMessage, ClientMessage>::new(transport, Config::default());

    loop {
        let events: Vec<_> = server.poll(Instant::now()).collect();
        for ev in events {
            match ev {
                Event::Connected(c) => println!("{c:?} connected"),
                Event::Message(c, ClientMessage::Hello { version, name }) => {
                    println!("{c:?} joined as {name}");
                }
                Event::Message(_, ClientMessage::Input { .. }) => {}
                Event::Disconnected(c, why) => println!("{c:?} left: {why:?}"),
            }
        }
        server
            .transport_mut()
            .wait(std::time::Duration::from_millis(2));
    }
}
