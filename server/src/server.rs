use std::time::Instant;

use shared::{
    coll::Vec2,
    net::{
        Channel, ClientMessage, ConnId, Endpoint, Event, ServerMessage, Transport,
        proto::{PROTO_VERSION, RejectReason, Snapshot, Welcome},
    },
    sim::{CarState, Input, TICK_RATE},
};

pub const SNAPSHOT_EVERY: u32 = 2;

pub struct Server<T: Transport> {
    pub net: Endpoint<T, ServerMessage, ClientMessage>,
    pub game: Game,
}

pub struct Game {
    clients: Vec<Client>,
    lobbies: Vec<Lobby>,
}

pub struct Client {
    id: ConnId,
    name: String,
    handshaked: bool,

    lobby: Option<usize>,
    lobby_slot: usize,

    pending_inputs: Vec<Input>,
    last_input: Input,
    last_seq_applied: u32,

    car: CarState,
}

pub struct Lobby {
    private: bool,
    code: Option<String>,

    clients: Vec<Option<ConnId>>,
    tick: u32,
}

impl<T> Server<T>
where
    T: Transport,
{
    pub fn poll_network(&mut self) {
        let events: Vec<_> = self.net.poll(Instant::now()).collect();

        for event in events {
            match event {
                Event::Connected(client) => {
                    self.game.connected(client);
                }

                Event::Disconnected(client, why) => {
                    self.game.disconnected(client, why);
                }

                Event::Message(client, message) => {
                    self.handle_message(client, message);
                }
            }
        }
    }
    fn handle_message(&mut self, client: ConnId, message: ClientMessage) {
        let events = match message {
            ClientMessage::Hello { version, name } => self.game.hello(client, version, name),

            ClientMessage::Input { inputs } => {
                self.game.input(client, inputs);
                Vec::new()
            }

            ClientMessage::CreateLobby => self.game.create_lobby(client),

            ClientMessage::JoinLobby { code } => self.game.join_lobby(client, code),

            ClientMessage::Quickplay => self.game.quickplay(client),
        };

        self.send_events(events);
    }
    fn send_events(&mut self, events: Vec<GameEvent>) {
        for event in events {
            match event {
                GameEvent::Send {
                    client,
                    channel,
                    message,
                } => {
                    let _ = self.net.send(client, channel, &message);
                }

                GameEvent::Disconnect(client) => {
                    let _ = self.net.disconnect(client);
                }
            }
        }
    }
    pub fn tick(&mut self) {
        let outgoing = self.game.tick();

        for event in outgoing {
            match event {
                GameEvent::Send {
                    client,
                    channel,
                    message,
                } => {
                    let _ = self.net.send(client, channel, &message);
                }

                GameEvent::Disconnect(client) => {
                    let _ = self.net.disconnect(client);
                }
            }
        }
    }
}

enum GameEvent {
    Send {
        client: ConnId,
        channel: Channel,
        message: ServerMessage,
    },

    Disconnect(ConnId),
}

impl Game {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
            lobbies: Vec::new(),
        }
    }

    fn connected(&mut self, id: ConnId) {
        self.clients.push(Client {
            id,
            name: String::new(),
            handshaked: false,
            lobby: None,
            lobby_slot: 0,
            pending_inputs: Vec::new(),
            last_input: Input::default(),
            last_seq_applied: 0,
            car: CarState::default(),
        });

        println!("{id:?} connected");
    }
    fn disconnected(&mut self, id: ConnId, why: impl std::fmt::Debug) {
        println!("{id:?} disconnected: {why:?}");

        let Some(client) = self.clients.iter().position(|c| c.id == id) else {
            return;
        };

        self.remove_from_lobby(client);
        self.clients.remove(client);
    }

    fn hello(&mut self, id: ConnId, version: u8, name: String) -> Vec<GameEvent> {
        if version != PROTO_VERSION {
            return vec![
                GameEvent::Send {
                    client: id,
                    channel: Channel::ReliableUnordered,
                    message: ServerMessage::Reject(RejectReason::VersionMismatch),
                },
                GameEvent::Disconnect(id),
            ];
        }

        let Some(client) = self.client_mut(id) else {
            return Vec::new();
        };

        client.name = name;
        client.handshaked = true;

        Vec::new()
    }
    fn input(&mut self, id: ConnId, inputs: Vec<Input>) {
        let Some(client) = self.client_mut(id) else {
            return;
        };

        if client.lobby.is_none() {
            return;
        }

        client.pending_inputs.extend(inputs);
    }
    fn quickplay(&mut self, id: ConnId) -> Vec<GameEvent> {
        let Some(lobby) = self.find_public_lobby() else {
            let lobby = self.create_public_lobby();
            return self.join_lobby_internal(id, lobby);
        };

        self.join_lobby_internal(id, lobby)
    }
    fn create_lobby(&mut self, id: ConnId) -> Vec<GameEvent> {
        let lobby = self.create_private_lobby();
        self.join_lobby_internal(id, lobby)
    }
    fn join_lobby(&mut self, id: ConnId, code: String) -> Vec<GameEvent> {
        let Some(lobby) = self
            .lobbies
            .iter()
            .position(|l| l.private && l.code.as_deref() == Some(code.as_str()))
        else {
            return vec![GameEvent::Send {
                client: id,
                channel: Channel::ReliableUnordered,
                message: ServerMessage::Reject(RejectReason::LobbyNotFound),
            }];
        };

        self.join_lobby_internal(id, lobby)
    }

    fn tick(&mut self) -> Vec<GameEvent> {
        let mut events = Vec::new();

        for lobby_index in 0..self.lobbies.len() {
            if self.lobbies[lobby_index].clients.is_empty() {
                continue;
            }

            self.lobby_tick(lobby_index);

            if self.lobbies[lobby_index].tick % SNAPSHOT_EVERY == 0 {
                self.send_snapshot(lobby_index, &mut events);
            }
        }

        events
    }
    fn lobby_tick(&mut self, lobby_index: usize) {
        let client_ids: Vec<ConnId> = self.lobbies[lobby_index]
            .clients
            .iter()
            .flatten()
            .copied()
            .collect();

        for id in &client_ids {
            let Some(client) = self.client_mut(*id) else {
                continue;
            };

            if let Some(input) = client.pending_inputs.pop() {
                client.last_seq_applied = input.seq;
                client.last_input = input;
            }

            client.car.step(&client.last_input);
            // level_resolve_car(s->level, &c->car);
        }

        // todo: car-car collisions

        self.lobbies[lobby_index].tick += 1;
    }

    fn send_snapshot(&self, lobby_index: usize, events: &mut Vec<GameEvent>) {
        let lobby = &self.lobbies[lobby_index];

        for client_id in lobby.clients.iter().flatten() {
            let Some(client) = self.client(*client_id) else {
                continue;
            };

            let ents: Vec<(u8, CarState)> = lobby
                .clients
                .iter()
                .enumerate()
                .filter_map(|(slot, id)| {
                    let id = (*id)?;
                    let client = self.client(id)?;

                    Some((slot as u8, client.car))
                })
                .collect();

            let snapshot = ServerMessage::Snapshot(Snapshot {
                server_tick: lobby.tick,
                ack_seq: client.last_seq_applied,
                ents: ents,
            });

            events.push(GameEvent::Send {
                client: *client_id,
                channel: Channel::Unreliable,
                message: snapshot,
            });
        }
    }

    fn join_lobby_internal(&mut self, id: ConnId, lobby_index: usize) -> Vec<GameEvent> {
        let Some(slot) = self.lobbies[lobby_index]
            .clients
            .iter()
            .position(Option::is_none)
        else {
            return vec![GameEvent::Send {
                client: id,
                channel: Channel::ReliableUnordered,
                message: ServerMessage::Reject(RejectReason::LobbyFull),
            }];
        };

        let lobby_code = self.lobbies[lobby_index]
            .code
            .clone()
            .unwrap_or(String::new());

        let Some(client) = self.client_mut(id) else {
            return Vec::new();
        };

        client.lobby = Some(lobby_index);
        client.lobby_slot = slot;

        client.car = CarState {
            pos: Vec2 {
                x: 100. + slot as f32 * 40.,
                y: 100.,
            },
            ..Default::default()
        };

        let welcome = ServerMessage::Welcome(Welcome {
            your_id: slot as u8,
            tick_rate: TICK_RATE,
            spawn: client.car,
            lobby_code,
        });

        self.lobbies[lobby_index].clients[slot] = Some(id);

        vec![GameEvent::Send {
            client: id,
            channel: Channel::ReliableUnordered,
            message: welcome,
        }]
    }
    fn remove_from_lobby(&mut self, client_index: usize) {
        let Some(lobby_index) = self.clients[client_index].lobby else {
            return;
        };

        let slot = self.clients[client_index].lobby_slot;

        self.lobbies[lobby_index].clients[slot] = None;

        if self.lobbies[lobby_index]
            .clients
            .iter()
            .all(Option::is_none)
        {
            self.lobbies.remove(lobby_index);
        }
    }
    fn create_public_lobby(&mut self) -> usize {
        self.lobbies.push(Lobby {
            private: false,
            code: None,
            clients: vec![None; 8],
            tick: 0,
        });

        self.lobbies.len() - 1
    }
    fn create_private_lobby(&mut self) -> usize {
        let code = format!("LOBBY-{}", self.lobbies.len());

        self.lobbies.push(Lobby {
            private: true,
            code: Some(code),
            clients: vec![None; 8],
            tick: 0,
        });

        self.lobbies.len() - 1
    }
    fn find_public_lobby(&self) -> Option<usize> {
        self.lobbies
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.private)
            .filter(|(_, l)| l.clients.iter().any(Option::is_none))
            .max_by_key(|(_, l)| l.clients.iter().flatten().count())
            .map(|(i, _)| i)
    }

    fn client(&self, id: ConnId) -> Option<&Client> {
        self.clients.iter().find(|c| c.id == id)
    }
    fn client_mut(&mut self, id: ConnId) -> Option<&mut Client> {
        self.clients.iter_mut().find(|c| c.id == id)
    }
}
