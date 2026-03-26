import { createTRPCClient, createWSClient, httpBatchLink, wsLink } from "@trpc/client";
import type { AppRouter } from "@wrum/server";

const apiUrl = import.meta.env.VITE_API_URL;
const wsUrl = import.meta.env.VITE_WS_URL;

export const wsClient = createWSClient({
  url: wsUrl,
});

export const trpc = createTRPCClient<AppRouter>({
  links: [
    httpBatchLink({
      url: apiUrl,
    }),
    wsLink({
      client: wsClient,
    }),
  ],
});
