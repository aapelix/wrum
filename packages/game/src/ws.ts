import type { ClientMessage } from "@wrum/shared";

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
}

export function send(msg: ClientMessage) {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  } else {
    console.warn("WebSocket is not open. Message not sent:", msg);
  }
}
