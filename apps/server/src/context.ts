import type { CreateFastifyContextOptions } from "@trpc/server/adapters/fastify";
import { auth } from "./auth/lib";
import { fromNodeHeaders } from "better-auth/node";

type GameContext = {
  lobbyId?: string;
  playerId?: string;
};

export const players = new Map<string, GameContext>();

export function setPlayerContext(userId: string, context: GameContext) {
  players.set(userId, context);
}

export function clearPlayerContext(userId: string) {
  players.delete(userId);
}

export async function createContext({ req, res }: CreateFastifyContextOptions) {
  const session = await auth.api.getSession({
    headers: fromNodeHeaders(req.headers),
  });

  if (!session?.user) {
    return { req, res, gameContext: {} as GameContext };
  }

  const userId = session.user.id;
  let gameContext = players.get(userId);

  if (!gameContext) {
    gameContext = {};
    players.set(userId, gameContext);
  }

  return { req, res, userId, gameContext };
}
export type Context = Awaited<ReturnType<typeof createContext>>;
