import { procedure, router } from "../trpc";
import { createSchema, joinSchema } from "@wrum/shared";
import { createLobby, getLobby, joinLobby, leaveLobby } from "./lobby";
import { TRPCError } from "@trpc/server";

const chars = "abcdefghijklmnopqrstuvwxyz0123456789";

function randomCode(length = 6) {
  return Array.from({ length }, () => chars[Math.floor(Math.random() * chars.length)]).join("");
}

export const gameRouter = router({
  join: procedure.input(joinSchema).mutation(({ input, ctx }) => {
    const playerId = joinLobby(input.lobbyId, input.carType);
    ctx.gameContext.lobbyId = input.lobbyId;
    ctx.gameContext.playerId = playerId;

    return playerId;
  }),
  create: procedure.input(createSchema).mutation(({ input, ctx }) => {
    let lobbyId = randomCode();

    let attempt = 0;
    if (getLobby(lobbyId)) {
      while (attempt < 5) {
        const newLobbyId = randomCode();
        if (!getLobby(newLobbyId)) {
          lobbyId = newLobbyId;
          break;
        }
        attempt++;
      }
      if (attempt === 5) {
        throw new TRPCError({
          code: "INTERNAL_SERVER_ERROR",
          message: "Failed to create lobby",
        });
      }
    }

    createLobby(lobbyId);

    const playerId = joinLobby(lobbyId, input.carType);
    ctx.gameContext.lobbyId = lobbyId;
    ctx.gameContext.playerId = playerId;

    return { playerId, lobbyId };
  }),
  leave: procedure.mutation(({ ctx }) => {
    const { lobbyId, playerId } = ctx.gameContext;
    if (!lobbyId || !playerId) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Not in a lobby",
      });
    }

    leaveLobby(lobbyId, playerId);
  }),
});
