import type { ServerWebSocket } from "bun";
import type { WebSocketData } from "../../ws";
import { lobbies, removeLobby } from "../lobby";

export function leave(ws: ServerWebSocket<WebSocketData>) {
  const lobbyId = ws.data.lobbyId;
  if (!lobbyId) return;

  const lobby = lobbies.get(lobbyId);
  if (!lobby) return;

  lobby.players.delete(ws.data.user.id);

  if (lobby.players.size === 0) {
    removeLobby(lobbyId);
  }
}
