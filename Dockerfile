# syntax=docker/dockerfile:experimental
FROM rust:1-alpine3.16 as builder
RUN apk add --no-cache cargo pkgconfig openssl openssl-dev
ENV HOME=/root
WORKDIR /app/
COPY . /app/
ARG GIT_HASH
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/root/app/target GIT_HASH=${GIT_HASH} cargo build --target=x86_64-unknown-linux-musl --release --color never
RUN ls /app/target/x86_64-unknown-linux-musl/release/

FROM alpine:3.16
LABEL org.opencontainers.image.source=https://github.com/ngerakines/something
LABEL org.opencontainers.image.description="A mutating webhook that writes consul key checksums to resoures."
RUN apk add --no-cache openssl
ENV RUST_LOG="warning"
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/k8s-consul-mutator-rs /usr/local/bin/k8s-consul-mutator-rs
CMD ["sh", "-c", "/usr/local/bin/k8s-consul-mutator-rs"]
