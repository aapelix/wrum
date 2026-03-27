import { createTRPCClient, createWSClient, httpBatchLink, splitLink, wsLink } from "@trpc/client";
import { msgpackEncoder } from "@wrum/encoder";
import type { AppRouter } from "@wrum/server";

const trpcUrl = import.meta.env.VITE_TRPC_URL;
const wsUrl = import.meta.env.VITE_WS_URL;

export const wsClient = createWSClient({
  url: wsUrl,
  experimental_encoder: msgpackEncoder,
});

export const trpc = createTRPCClient<AppRouter>({
  links: [
    splitLink({
      condition(op) {
        return op.type === "subscription";
      },
      true: wsLink({
        client: wsClient,
      }),
      false: httpBatchLink({
        url: trpcUrl,
      }),
    }),
  ],
});
