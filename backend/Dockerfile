FROM rust:1.88-slim AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/finance-radar-backend /usr/local/bin/finance-radar-backend

ENV HOST=0.0.0.0
ENV PORT=4000
EXPOSE 4000

CMD ["finance-radar-backend"]
