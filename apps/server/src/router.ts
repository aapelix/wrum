import { initTRPC } from "@trpc/server";

export const t = initTRPC.create();

export const appRouter = t.router({
  randomNumber: t.procedure.subscription(async function* () {
    while (true) {
      yield { randomNumber: Math.random() };
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }),
});

export type AppRouter = typeof appRouter;
