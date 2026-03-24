type WebSocketData = {
  createdAt: number;
  // id, authToken etc
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
  fetch() {
    return new Response("not found", {
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
