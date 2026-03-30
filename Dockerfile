FROM oven/bun:latest AS build
WORKDIR /app
COPY . .
RUN bun install
RUN bun run --cwd apps/game build

FROM caddy:alpine
COPY --from=build /app/apps/game/dist /srv
COPY Caddyfile /etc/caddy/Caddyfile