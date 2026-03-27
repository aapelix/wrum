import { z } from "zod";

export const carTypes = ["red"] as const;

export const carType = z.enum(carTypes);
export type CarType = z.infer<typeof carType>;

export const joinSchema = z.object({
  lobbyId: z
    .string()
    .length(6)
    .regex(/^[a-z0-9]+$/),
  carType: carType,
});

export const createSchema = z.object({
  carType: carType,
});

export type JoinSchema = z.infer<typeof joinSchema>;
export type CreateSchema = z.infer<typeof createSchema>;

// export type Player = {
//   id: string;
//   carType: CarType;
//   x: number;
//   y: number;
//   rotation: number;
// };

// export type ClientJoinLobbyData = { carType: CarType; lobbyId: string };
// export type ClientCreateLobbyData = { carType: CarType };
// export type ClientLeaveData = { reason: string };
// export type ClientUpdateData = { throttle: number; steering: number };

// export type ClientMessage =
//   | { type: "join"; data: ClientJoinLobbyData }
//   | { type: "create"; data: ClientCreateLobbyData }
//   | { type: "leave"; data: ClientLeaveData }
//   | { type: "update"; data: ClientUpdateData };

// export type ServerUpdateData = {
//   players: Player[];
// };

// export type ServerMessage =
//   | { type: "update"; data: ServerUpdateData }
//   | {
//       type: "created";
//       data: {
//         lobbyId: string;
//       };
//     }
//   | {
//       type: "join";
//       data: {
//         lobbyId: string;
//         players: Player[];
//       };
//     }
//   | {
//       type: "otherJoined";
//       data: {
//         player: Player;
//       };
//     }
//   | { type: "error"; data: { message: string } };
