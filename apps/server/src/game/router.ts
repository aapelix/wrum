import { authProcedure, router } from "../trpc";
import { createSchema, inputSchema, joinSchema, type ServerMessage } from "@wrum/shared";
import {
  createLobby,
  getLobby,
  joinLobby,
  leaveLobby,
  lobbyEmitters,
  sendInput,
} from "./lobby/manager";
import { TRPCError } from "@trpc/server";
import { on } from "node:events";
import { clearPlayerContext, players, setPlayerContext } from "../context";

const chars = "abcdefghijklmnopqrstuvwxyz0123456789";

function randomCode(length = 6) {
  return Array.from({ length }, () => chars[Math.floor(Math.random() * chars.length)]).join("");
}

export const gameRouter = router({
  join: authProcedure.input(joinSchema).mutation(({ input, ctx }) => {
    const playerId = joinLobby(input.lobbyId, input.carType);
    setPlayerContext(ctx.userId, {
      lobbyId: input.lobbyId,
      playerId,
    });

    return playerId;
  }),
  create: authProcedure.input(createSchema).mutation(({ input, ctx }) => {
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
    setPlayerContext(ctx.userId, {
      lobbyId,
      playerId,
    });

    return { playerId, lobbyId };
  }),
  leave: authProcedure.mutation(({ ctx }) => {
    const { lobbyId, playerId } = ctx.gameContext;
    if (!lobbyId || !playerId) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Not in a lobby",
      });
    }

    leaveLobby(lobbyId, playerId);

    clearPlayerContext(ctx.userId);
  }),
  clientUpdate: authProcedure.input(inputSchema).mutation(({ ctx, input }) => {
    const gameCtx = players.get(ctx.userId);
    if (!gameCtx) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Not in a lobby",
      });
    }

    const { lobbyId, playerId } = gameCtx;

    if (!lobbyId || !playerId) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Not in a lobby",
      });
    }

    sendInput(lobbyId, playerId, input);
  }),
  serverUpdate: authProcedure.subscription(async function* ({ ctx, signal }) {
    const gameCtx = players.get(ctx.userId);
    const lobbyId = gameCtx?.lobbyId;

    if (!lobbyId) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Not in a lobby",
      });
    }

    const ee = lobbyEmitters.get(lobbyId);
    if (!ee) {
      throw new TRPCError({
        code: "NOT_FOUND",
        message: "Lobby not found",
      });
    }

    const onLeave = () => {
      ee.emit("message", {
        type: "leave",
        data: {
          playerId: gameCtx.playerId!,
        },
      } as ServerMessage);
    };

    signal?.addEventListener("abort", onLeave);

    for await (const [msg] of on(ee, "message", { signal }) as AsyncIterable<[ServerMessage]>) {
      yield msg;
    }
  }),
});
