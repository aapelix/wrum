import type { ClientMessage, ServerMessage } from "@wrum/shared";

let ws: WebSocket;

export function initWs() {
  if (ws) {
    return;
  }

  const wsUrl = import.meta.env.VITE_WS_URL;

  if (!wsUrl) {
    throw new Error("VITE_WS_URL is not defined");
  }

  ws = new WebSocket(`${wsUrl}/conn`);

  ws.addEventListener("open", () => {
    debug.log("WebSocket connection opened");
  });

  ws.addEventListener("message", (m) => {
    const msg = JSON.parse(m.data) as ServerMessage;
    switch (msg.type) {
      case "join":
        go("lobby", msg.data.lobbyId);
        break;
      case "update":
        break;
      case "created":
        go("lobby", msg.data.lobbyId);
        break;
      case "error":
        go("error", msg.data.message);
        console.error(msg);
        break;
    }
  });
}

export function send(msg: ClientMessage) {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  } else {
    console.warn("WebSocket is not open. Message not sent:", msg);
  }
}
