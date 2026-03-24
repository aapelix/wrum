import type { ClientJoinData } from "@wrum/shared";
import { createLobby, lobbies } from "../lobby";
import type { WebSocketData } from "../../ws";
import type { ServerWebSocket } from "bun";

export function join(ws: ServerWebSocket<WebSocketData>, data: ClientJoinData) {
  const lobbyId = data.lobbyId;

  let lobby = lobbies.get(lobbyId);
  if (!lobby) {
    lobby = createLobby(lobbyId);
  }

  lobby.players.set(ws.data.user.id, {
    id: crypto.randomUUID(),
    ws,
    input: {
      throttle: 0,
      steering: 0,
    },
    carType: data.carType,
    x: 0,
    y: 0,
    rotation: 0,
  });

  ws.data.lobbyId = lobbyId;
}
