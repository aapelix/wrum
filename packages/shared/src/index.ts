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
  tireRotation: z.number(),
});

export type Player = z.infer<typeof playerSchema>;

export const serverMessageSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("update"),
    data: z.object({
      players: z.array(playerSchema),
    }),
  }),
  z.object({
    type: z.literal("join"),
    data: z.object({
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
      lobbyId: z.string(),
    }),
  }),
  z.object({
    type: z.literal("leave"),
    data: z.object({
      playerId: z.string(),
    }),
  }),
]);

export type ServerMessage = z.infer<typeof serverMessageSchema>;
