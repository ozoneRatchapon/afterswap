# Build stage
FROM rust:1-slim AS build
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
# katgpt-ruliology resolves as a pinned public git dependency.
RUN cargo build --release -p afterswap-server

# Runtime stage
FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/afterswap-server /usr/local/bin/afterswap-server
COPY data /app/data
ENV RUST_LOG=info
EXPOSE 8787
# Live DFlow quotes; judges open positions themselves from the dashboard.
CMD ["afterswap-server", "--serve", "8787", "--interval-ms", "1000", "--window", "12", "--states", "3", "--tranche", "0.1"]
