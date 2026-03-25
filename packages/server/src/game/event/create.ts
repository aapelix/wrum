import type { ClientCreateLobbyData, ServerMessage } from "@wrum/shared";
import { createLobby, lobbies } from "../lobby";
import type { WebSocketData } from "../../ws";
import type { ServerWebSocket } from "bun";

const chars = "abcdefghijklmnopqrstuvwxyz0123456789";

function randomCode(length = 6) {
  return Array.from(
    { length },
    () => chars[Math.floor(Math.random() * chars.length)]
  ).join("");
}

export function create(
  ws: ServerWebSocket<WebSocketData>,
  data: ClientCreateLobbyData,
  attempt: number = 0
) {
  if (attempt > 10) {
    ws.send(
      JSON.stringify({
        type: "error",
        data: {
          message: "Failed to create lobby",
        },
      } as ServerMessage)
    );
  }

  const code = randomCode();
  if (lobbies.get(code)) {
    create(ws, data, attempt + 1);
  }

  const lobby = createLobby(code);

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

  ws.data.lobbyId = lobby.id;

  ws.send(
    JSON.stringify({
      type: "created",
      data: {
        lobbyId: lobby.id,
      },
    } as ServerMessage)
  );
}
