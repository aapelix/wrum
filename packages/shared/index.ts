export const carTypes = ["red"] as const;
export type CarType = (typeof carTypes)[number];

export type ClientJoinData = { carType: CarType; lobbyId: string };
export type ClientLeaveData = { reason: string };
export type ClientUpdateData = { throttle: number; steering: number };

export type ClientMessage =
  | { type: "join"; data: ClientJoinData }
  | { type: "leave"; data: ClientLeaveData }
  | { type: "update"; data: ClientUpdateData };

// server join, leave etc...
export type ServerUpdateData = {
  players: {
    id: string;
    carType: CarType;
    x: number;
    y: number;
    rotation: number;
  }[];
};

export type ServerMessage =
  | { type: "update"; data: ServerUpdateData }
  | { type: "error"; data: { message: string } };
