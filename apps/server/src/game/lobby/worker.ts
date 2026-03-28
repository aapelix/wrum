import type { CarType, InputSchema, ServerMessage } from "@wrum/shared";

type Player = {
  id: string;
  input: InputSchema;
  x: number;
  y: number;
  rotation: number;
  carType: CarType;
};

type Lobby = {
  players: Map<string, Player>;
  lastTick: number;
};

const lobbies = new Map<string, Lobby>();

self.onmessage = (event) => {
  const msg = event.data;

  if (msg.type === "create") {
    lobbies.set(msg.lobbyId, {
      players: new Map(),
      lastTick: Date.now(),
    });

    startLoop(msg.lobbyId);
  }

  if (msg.type === "join") {
    const lobby = lobbies.get(msg.lobbyId);
    if (!lobby) return;

    const x = 0;
    const y = 0;
    const rotation = 0;

    lobby.players.set(msg.playerId, {
      id: msg.playerId,
      input: {
        throttle: 0,
        steering: 0,
      },
      x,
      y,
      rotation,
      carType: msg.carType,
    });

    post(msg.lobbyId, {
      type: "join",
      data: {
        players: Array.from(lobby.players.values()).map((p) => ({
          id: p.id,
          x: p.x,
          y: p.y,
          rotation: p.rotation,
          carType: p.carType,
        })),
      },
    });

    post(msg.lobbyId, {
      type: "otherJoined",
      data: {
        player: {
          id: msg.playerId,
          x,
          y,
          rotation,
          carType: msg.carType,
        },
      },
    });
  }

  if (msg.type === "leave") {
    const lobby = lobbies.get(msg.lobbyId);
    if (!lobby) return;

    lobby.players.delete(msg.playerId);

    post(msg.lobbyId, {
      type: "leave",
      data: {
        playerId: msg.playerId,
      },
    });

    if (lobby.players.size === 0) {
      lobbies.delete(msg.lobbyId);

      post(msg.lobbyId, { type: "close", data: { lobbyId: msg.lobbyId } });
    }
  }

  if (msg.type === "input") {
    const lobby = lobbies.get(msg.lobbyId);
    if (!lobby) return;
    const player = lobby.players.get(msg.playerId);
    if (player) player.input = msg.input;
  }
};

const tickRate = 50;

function startLoop(lobbyId: string) {
  setInterval(() => {
    const lobby = lobbies.get(lobbyId);
    if (!lobby) return;

    const now = Date.now();
    const dt = (now - lobby.lastTick) / 1000;
    lobby.lastTick = now;

    for (const player of lobby.players.values()) {
      // Simple physics for demonstration
      const speed = player.input.throttle * 100; // 100 units per second at full throttle
      player.x += Math.cos(player.rotation) * speed * dt;
      player.y += Math.sin(player.rotation) * speed * dt;
      player.rotation += player.input.steering * dt; // steering input directly changes rotation

      // Keep rotation between 0 and 2*PI
      if (player.rotation < 0) player.rotation += 2 * Math.PI;
      if (player.rotation >= 2 * Math.PI) player.rotation -= 2 * Math.PI;
    }

    post(lobbyId, {
      type: "update",
      data: {
        lobbyId,
        players: Array.from(lobby.players.values()).map((p) => ({
          id: p.id,
          x: p.x,
          y: p.y,
          rotation: p.rotation,
          carType: p.carType,
        })),
      },
    } as ServerMessage);
  }, tickRate);
}

function post(lobbyId: string, msg: ServerMessage) {
  self.postMessage({ lobbyId, msg });
}
