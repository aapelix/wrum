import type { ServerWebSocket } from "bun";
import type { WebSocketData } from "../ws";
import type { ClientMessage } from "@wrum/shared";
import { join } from "./event/join";
import { update } from "./event/update";
import { leave } from "./event/leave";
import { create } from "./event/create";

export function handleMsg(
  ws: ServerWebSocket<WebSocketData>,
  msg: ClientMessage
) {
  switch (msg.type) {
    case "join": {
      join(ws, msg.data);
      break;
    }
    case "update": {
      update(ws, msg.data);
      break;
    }
    case "leave": {
      leave(ws);
      break;
    }
    case "create": {
      console.log("a");
      create(ws, msg.data);
      break;
    }
  }
}
