import type { CarType } from "@wrum/shared";
import type { Player } from "./player";
import { TRPCError } from "@trpc/server";

export const lobbies = new Map<string, Lobby>();

export type Lobby = {
  id: string;
  players: Map<string, Player>;
  //state: any; // TODO: define this properly
  lastTick: number;
};

export function createLobby(id: string): Lobby {
  const lobby = {
    id,
    players: new Map(),
    //state: {},
    lastTick: Date.now(),
  };

  lobbies.set(id, lobby);
  return lobby;
}

export function joinLobby(lobbyId: string, carType: CarType): string {
  const lobby = lobbies.get(lobbyId);
  if (!lobby) {
    throw new TRPCError({
      code: "NOT_FOUND",
      message: "Lobby not found",
    });
  }

  const playerId = crypto.randomUUID();
  lobby.players.set(playerId, {
    id: playerId,
    input: {
      throttle: 0,
      steering: 0,
    },
    x: 0,
    y: 0,
    rotation: 0,
    carType,
  });

  return playerId;
}

export function getLobby(id: string): Lobby | undefined {
  return lobbies.get(id);
}

export function leaveLobby(lobbyId: string, playerId: string) {
  const lobby = lobbies.get(lobbyId);
  if (!lobby) {
    throw new TRPCError({
      code: "NOT_FOUND",
      message: "Lobby not found",
    });
  }

  lobby.players.delete(playerId);

  if (lobby.players.size === 0) {
    lobbies.delete(lobbyId);
  }
}

export function removeLobby(id: string) {
  lobbies.delete(id);
}

export function lobbyState(lobby: Lobby) {
  return {
    players: Array.from(lobby.players.values()).map((player) => ({
      id: player.id,
      x: player.x,
      y: player.y,
      rotation: player.rotation,
      carType: player.carType,
    })),
  };
}

export function startLobbyLoop(lobby: Lobby) {
  const tickRate = 50; // ms = 20 ticks per second

  const loop = () => {
    if (!lobbies.has(lobby.id)) {
      return;
    }

    const now = Date.now();
    //const dt = now - lobby.lastTick;
    lobby.lastTick = now;

    // step physics w/ dt

    // const snapshot = lobbyState(lobby);

    // for (const player of lobby.players.values()) {
    //   player.ws.send(msg);
    // }

    setTimeout(loop, tickRate);
  };

  loop();
}
