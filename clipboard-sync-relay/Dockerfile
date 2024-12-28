FROM rust:latest as build-env

WORKDIR /app
COPY . .

RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=build-env /app/target/release/clipboard-sync-relay /
CMD ["./clipboard-sync-relay"]
