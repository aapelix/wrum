import type { ServerMessage } from "@wrum/shared";
import type { Player } from "./player";
import { handleCollisions } from "../physics/aabb";

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

      // these probably should be based on car type at some point from db or something
      velocity: 0,
      acceleration: 100,
      maxVelocity: 150,
      friction: 50,

      turnSpeed: 20,

      tireRotation: 0,
      tireTurnSpeed: 70,
      tireReturnSpeed: 70,
      tireMaxRotation: 30,

      collider: {
        width: 8,
        height: 15,
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
          tireRotation: 0,
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
      if (player.input.throttle !== 0) {
        player.velocity += player.acceleration * player.input.throttle * dt;
      } else {
        if (player.velocity > 0) {
          player.velocity -= player.friction * dt;
          if (player.velocity < 0) player.velocity = 0;
        } else if (player.velocity < 0) {
          player.velocity += player.friction * dt;
          if (player.velocity > 0) player.velocity = 0;
        }
      }

      player.velocity = Math.max(
        -player.maxVelocity,
        Math.min(player.maxVelocity, player.velocity),
      );

      if (player.input.steering !== 0) {
        player.tireRotation += player.tireTurnSpeed * player.input.steering * dt;
      } else {
        if (player.tireRotation > 0) {
          player.tireRotation -= player.tireReturnSpeed * dt;
          if (player.tireRotation < 0) player.tireRotation = 0;
        } else if (player.tireRotation < 0) {
          player.tireRotation += player.tireReturnSpeed * dt;
          if (player.tireRotation > 0) player.tireRotation = 0;
        }
      }

      player.tireRotation = Math.max(
        -player.tireMaxRotation,
        Math.min(player.tireMaxRotation, player.tireRotation),
      );

      const rotationRad = (player.rotation * Math.PI) / 180;

      player.rotation +=
        player.tireRotation * (player.velocity / player.maxVelocity) * player.turnSpeed * dt;

      player.x += Math.sin(rotationRad) * player.velocity * dt;
      player.y -= Math.cos(rotationRad) * player.velocity * dt;
    }

    handleCollisions(Array.from(lobby.players.values()));

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
          tireRotation: p.tireRotation,
        })),
      },
    } as ServerMessage);
  }, tickRate);
}

function post(lobbyId: string, msg: ServerMessage) {
  self.postMessage({ lobbyId, msg });
}
