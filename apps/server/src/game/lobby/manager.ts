import { TRPCError } from "@trpc/server";
import { getWorker, workers } from "./pool";
import type { CarType, InputSchema, ServerMessage } from "@wrum/shared";
import { EventEmitter } from "node:events";

export const lobbies = new Map<
  string,
  {
    id: string;
    worker: Worker;
  }
>();

export const lobbyEmitters = new Map<string, EventEmitter>();

function getLobbyEmitter(lobbyId: string) {
  let ee = lobbyEmitters.get(lobbyId);
  if (!ee) {
    ee = new EventEmitter();
    lobbyEmitters.set(lobbyId, ee);
  }
  return ee;
}

function emitLobby(lobbyId: string, data: ServerMessage) {
  const ee = getLobbyEmitter(lobbyId);
  ee.emit("message", data);
}

for (const worker of workers) {
  worker.onmessage = (event) => {
    const msg = event.data as { lobbyId: string; msg: ServerMessage };

    if (msg.msg.type === "update") {
      emitLobby(msg.lobbyId, msg.msg);
    } else if (msg.msg.type === "close") {
      const { lobbyId } = msg.msg.data;
      lobbies.delete(lobbyId);
      emitLobby(msg.lobbyId, msg.msg);
    } else {
      emitLobby(msg.lobbyId, msg.msg);
    }
  };
}

export function createLobby(id: string) {
  const worker = getWorker(id);
  if (!worker) {
    throw new TRPCError({
      code: "INTERNAL_SERVER_ERROR",
      message: "No available worker",
    });
  }

  worker.postMessage({
    type: "create",
    lobbyId: id,
  });

  lobbies.set(id, {
    id,
    worker,
  });

  getLobbyEmitter(id);
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

  lobby.worker.postMessage({
    type: "join",
    lobbyId,
    playerId,
    carType,
  });

  return playerId;
}

export function leaveLobby(lobbyId: string, playerId: string) {
  const lobby = lobbies.get(lobbyId);
  if (!lobby) return;

  lobby.worker.postMessage({
    type: "leave",
    lobbyId,
    playerId,
  });
}

export function sendInput(lobbyId: string, playerId: string, input: InputSchema) {
  const lobby = lobbies.get(lobbyId);
  if (!lobby) return;

  lobby.worker.postMessage({
    type: "input",
    lobbyId,
    playerId,
    input,
  });
}

export function getLobby(lobbyId: string) {
  return lobbies.get(lobbyId);
}
