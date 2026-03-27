import type { CreateFastifyContextOptions } from "@trpc/server/adapters/fastify";

type GameContext = {
  lobbyId?: string;
  playerId?: string;
};

export function createContext({ req, res }: CreateFastifyContextOptions) {
  return { req, res, gameContext: {} as GameContext };
}

export type Context = Awaited<ReturnType<typeof createContext>>;
