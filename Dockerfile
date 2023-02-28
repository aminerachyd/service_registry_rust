FROM rust:latest

WORKDIR /app

COPY . .

ENV REGISTRY_ADDR

RUN cargo build --release && mv ./target/release/processes processes && chmod +x processes

ENTRYPOINT ["./processes"]