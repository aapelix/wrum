import "dotenv/config";
import { auth } from "./lib/auth";
import type { ClientMessage } from "@wrum/shared";
import type { WebSocketData } from "./ws";
import { handleMsg } from "./game/msg";
import { leave } from "./game/event/leave";

const origin = process.env.CLIENT_URL!;

const s = Bun.serve({
  routes: {
    "/conn": async (req, server) => {
      const session = await auth.api.getSession({
        headers: req.headers,
      });

      if (!session) {
        return new Response("Unauthorized", {
          status: 401,
        });
      }

      if (
        server.upgrade(req, {
          data: {
            createdAt: Date.now(),
            user: {
              id: session.user.id,
              name: session.user.name,
            },
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
            "Access-Control-Allow-Origin": origin,
            "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
            "Access-Control-Allow-Headers": "Content-Type, Authorization",
            "Access-Control-Allow-Credentials": "true",
          },
        });
      }

      const res = await auth.handler(req);

      res.headers.set("Access-Control-Allow-Origin", origin);
      res.headers.set("Access-Control-Allow-Credentials", "true");

      return res;
    }

    return new Response("Not Found", {
      status: 404,
    });
  },
  websocket: {
    data: {} as WebSocketData,
    message(ws, msg) {
      if (typeof msg !== "string") {
        return;
      }

      const data = JSON.parse(msg) as ClientMessage;

      handleMsg(ws, data);
    },
    close(ws) {
      if (ws.data.lobbyId) {
        leave(ws);
      }
    },
  },
});

console.log(`Server running at ${s.hostname}:${s.port}`);
