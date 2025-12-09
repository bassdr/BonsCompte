FROM rust:1.89 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/bonscompte-backend /usr/local/bin/
EXPOSE 8000
CMD ["bonscompte-backend"]
