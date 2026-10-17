FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p wrum_server -j 2

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /src/target/release/wrum_server /app/wrum_server
COPY assets /app/assets
CMD ["/app/wrum_server"]
