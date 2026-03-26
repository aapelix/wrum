import type { ServerMessage, ServerUpdateData } from "@wrum/shared";
import type { Player } from "./player";

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

export function removeLobby(id: string) {
  lobbies.delete(id);
}

export function lobbyState(lobby: Lobby): ServerUpdateData {
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

    const snapshot = lobbyState(lobby);
    const msg = JSON.stringify({
      type: "update",
      data: snapshot,
    } as ServerMessage);

    for (const player of lobby.players.values()) {
      player.ws.send(msg);
    }

    setTimeout(loop, tickRate);
  };

  loop();
}
