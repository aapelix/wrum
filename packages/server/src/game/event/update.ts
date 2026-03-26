import type { ServerWebSocket } from "bun";
import type { WebSocketData } from "../../ws";
import type { ClientUpdateData } from "@wrum/shared";
import { lobbies } from "../lobby";

export function update(ws: ServerWebSocket<WebSocketData>, data: ClientUpdateData) {
  const lobbyId = ws.data.lobbyId;
  if (!lobbyId) {
    return;
  }

  const lobby = lobbies.get(lobbyId);
  if (!lobby) return;

  const player = lobby.players.get(ws.data.user.id);
  if (!player) return;

  player.input = data;
}
