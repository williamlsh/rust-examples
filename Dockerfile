FROM rust:alpine AS builder

RUN apk add --no-cache gcc musl-dev protobuf-dev

WORKDIR /app

COPY . .

RUN cargo build -F otel -r

FROM rust:alpine

COPY --from=builder /app/target/release/mini-redis-server /bin/
COPY --from=builder /app/target/release/mini-redis-cli /bin/

CMD [ "/bin/mini-redis-server" ]
