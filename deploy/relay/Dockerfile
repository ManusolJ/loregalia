FROM rust:1.99-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p loregalia-relay

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /data relay \
    && mkdir /data \
    && chown relay /data
COPY --from=build /src/target/release/loregalia-relay /usr/local/bin/loregalia-relay
USER relay
ENV LOREGALIA_RELAY_BIND=127.0.0.1:8080 \
    LOREGALIA_RELAY_DATABASE=/data/relay-cache.db \
    LOREGALIA_RELAY_TRUST_PROXY=1
VOLUME /data
CMD ["loregalia-relay"]
