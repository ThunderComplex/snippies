# BUILD
FROM rust:1.95-alpine AS builder

WORKDIR /application

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY frontend ./frontend

RUN cargo build --release

# RUN
FROM alpine:latest

WORKDIR /application

COPY --from=builder /application/target/release/snippies /usr/local/bin/snippies
COPY --from=builder /application/frontend ./frontend

RUN mkdir -p /application/snippies /application/output

EXPOSE 8192

VOLUME ["/application/snippies"]

CMD ["snippies", "--snippie-dir", "/application/snippies", "-o", "/application/output"]
