import type { ServerWebSocket } from "bun";
import type { WebSocketData } from "../ws";
import type { CarType } from "@wrum/shared";

export type Player = {
  id: string;
  ws: ServerWebSocket<WebSocketData>;
  input: {
    throttle: number;
    steering: number;
  };
  x: number;
  y: number;
  rotation: number;
  carType: CarType;
};
