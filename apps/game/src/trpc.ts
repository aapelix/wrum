import { createTRPCClient, createWSClient, httpBatchLink, splitLink, wsLink } from "@trpc/client";
import { msgpackEncoder } from "@wrum/encoder";
import type { AppRouter } from "server";

const trpcUrl = import.meta.env.VITE_TRPC_URL;
const wsUrl = import.meta.env.VITE_WS_URL;

console.log("tRPC URL:", trpcUrl);
console.log("WebSocket URL:", wsUrl);

export const wsClient = createWSClient({
  url: wsUrl,
  experimental_encoder: msgpackEncoder,
});

export const trpc = createTRPCClient<AppRouter>({
  links: [
    splitLink({
      condition(op) {
        return op.type === "subscription" || op.context?.useWS === true;
      },
      true: wsLink({
        client: wsClient,
      }),
      false: httpBatchLink({
        url: trpcUrl,
        fetch(url, options) {
          return fetch(url, {
            ...options,
            credentials: "include",
          });
        },
      }),
    }),
  ],
});
