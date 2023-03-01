FROM rust:latest

WORKDIR /app

COPY . .

RUN cargo build --release && mv ./target/release/processes processes && chmod +x processes

ENTRYPOINT ["./processes"]