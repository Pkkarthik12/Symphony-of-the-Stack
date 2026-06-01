FROM rust:1.83-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY crates ./crates
RUN cargo build --release -p symphony-bridge

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/symphony-bridge /usr/local/bin/symphony-bridge
COPY web /srv/web
ENV SYMPHONY_WEB_DIR=/srv/web
ENV SYMPHONY_HTTP_ADDR=0.0.0.0:8765
EXPOSE 8765
CMD ["symphony-bridge"]
