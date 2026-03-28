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

export const inputSchema = z.object({
  throttle: z.number().min(-1).max(1),
  steering: z.number().min(-1).max(1),
});

export type JoinSchema = z.infer<typeof joinSchema>;
export type CreateSchema = z.infer<typeof createSchema>;
export type InputSchema = z.infer<typeof inputSchema>;

export const playerSchema = z.object({
  id: z.string(),
  carType: carType,
  x: z.number(),
  y: z.number(),
  rotation: z.number(),
});

export type Player = z.infer<typeof playerSchema>;

export const serverUpdateSchema = z.object({
  lobbyId: z
    .string()
    .length(6)
    .regex(/^[a-z0-9]+$/),
  players: z.array(playerSchema),
});

export type ServerUpdate = z.infer<typeof serverUpdateSchema>;

export const serverMessageSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("update"),
    data: serverUpdateSchema,
  }),
  z.object({
    type: z.literal("created"),
    data: z.object({
      lobbyId: z
        .string()
        .length(6)
        .regex(/^[a-z0-9]+$/),
    }),
  }),
  z.object({
    type: z.literal("join"),
    data: z.object({
      lobbyId: z
        .string()
        .length(6)
        .regex(/^[a-z0-9]+$/),
      players: z.array(playerSchema),
    }),
  }),
  z.object({
    type: z.literal("otherJoined"),
    data: z.object({
      player: playerSchema,
    }),
  }),
  z.object({
    type: z.literal("error"),
    data: z.object({
      message: z.string(),
    }),
  }),
  z.object({
    type: z.literal("close"),
    data: z.object({
      lobbyId: z
        .string()
        .length(6)
        .regex(/^[a-z0-9]+$/),
    }),
  }),
]);

export type ServerMessage = z.infer<typeof serverMessageSchema>;

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
