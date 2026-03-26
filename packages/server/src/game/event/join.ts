import type { ClientJoinLobbyData, ServerMessage } from "@wrum/shared";
import { lobbies, lobbyState } from "../lobby";
import type { WebSocketData } from "../../ws";
import type { ServerWebSocket } from "bun";

export function join(ws: ServerWebSocket<WebSocketData>, data: ClientJoinLobbyData) {
  const lobbyId = data.lobbyId;

  const lobby = lobbies.get(lobbyId);
  if (!lobby) {
    ws.send(
      JSON.stringify({
        type: "error",
        data: {
          message: "No lobby found",
        },
      } as ServerMessage),
    );

    return;
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

  ws.send(
    JSON.stringify({
      type: "join",
      data: {
        lobbyId: lobbyId,
        players: lobbyState(lobby).players,
      },
    } as ServerMessage),
  );
}
