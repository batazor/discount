# discount-server: two-stage build. Context is the repository root.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY verification/drt verification/drt
COPY api api
RUN cargo build --release -p discount-infrastructure --bin discount-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/discount-server /usr/local/bin/discount-server
COPY deploy/promotions.json /etc/discount/promotions.json
ENV DISCOUNT_LISTEN=0.0.0.0:8080 \
    DISCOUNT_PROMOTIONS=/etc/discount/promotions.json \
    OTEL_EXPORTER_OTLP_ENDPOINT=http://lgtm:4317 \
    DISCOUNT_ENV=compose
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --retries=6 CMD curl -sf http://localhost:8080/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/discount-server"]
