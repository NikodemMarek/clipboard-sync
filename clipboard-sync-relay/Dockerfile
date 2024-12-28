FROM rust:latest

WORKDIR /usr/src/clipboard-sync-relay
COPY . .

RUN cargo install --path .

CMD ["clipboard-sync-relay"]
