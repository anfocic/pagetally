FROM rust:1.83-slim AS build
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release --bin pagetally-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/release/pagetally-server /usr/local/bin/pagetally-server
COPY --from=build /app/migrations /app/migrations
WORKDIR /app

EXPOSE 3001
CMD ["pagetally-server"]
