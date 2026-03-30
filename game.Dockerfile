FROM oven/bun:latest AS build
WORKDIR /app
COPY . .
RUN bun install

ARG VITE_API_URL
ARG VITE_TRPC_URL
ARG VITE_WS_URL

ENV VITE_API_URL=$VITE_API_URL
ENV VITE_TRPC_URL=$VITE_TRPC_URL
ENV VITE_WS_URL=$VITE_WS_URL

RUN bun run --cwd apps/game build

FROM caddy:alpine
COPY --from=build /app/apps/game/dist /srv
COPY Caddyfile /etc/caddy/Caddyfile