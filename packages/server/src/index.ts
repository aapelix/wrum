import "dotenv/config";
import { auth } from "./lib/auth";

type WebSocketData = {
  createdAt: number;
};

const s = Bun.serve({
  routes: {
    "/conn": (req, server) => {
      if (
        server.upgrade(req, {
          data: {
            createdAt: Date.now(),
          },
        })
      ) {
        return new Response("WebSocket connection established", {
          status: 101,
        });
      }
    },
  },
  async fetch(req) {
    const url = new URL(req.url);

    if (url.pathname.startsWith("/api/auth")) {
      if (req.method === "OPTIONS") {
        return new Response(null, {
          headers: {
            "Access-Control-Allow-Origin": "http://localhost:3001",
            "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
            "Access-Control-Allow-Headers": "Content-Type, Authorization",
            "Access-Control-Allow-Credentials": "true",
          },
        });
      }

      const res = await auth.handler(req);

      res.headers.set("Access-Control-Allow-Origin", "http://localhost:3001");
      res.headers.set("Access-Control-Allow-Credentials", "true");

      return res;
    }

    return new Response("Not Found", {
      status: 404,
    });
  },
  websocket: {
    data: {} as WebSocketData,
    open(ws) {
      console.log("WebSocket connection opened", ws.data);
    },
    message(ws, msg) {
      console.log("Received message:", ws, msg);
    },
    close(ws, code, reason) {
      console.log("WebSocket connection closed", ws, code, reason);
    },
  },
});

console.log(`Server running at ${s.hostname}:${s.port}`);
