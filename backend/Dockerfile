# Build stage
FROM rust:1.83-bookworm AS builder

WORKDIR /app

# Install SQLite dev libraries
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy source and rebuild
COPY . .
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 appuser

WORKDIR /app

# Copy binary and migrations
COPY --from=builder /app/target/release/bonscompte-backend /usr/local/bin/
COPY --from=builder /app/migrations ./migrations

# Create data directory for SQLite
RUN mkdir -p /data && chown appuser:appuser /data

USER appuser

EXPOSE 8000

ENV DATABASE_URL=sqlite:///data/bonscompte.db
ENV HOST=0.0.0.0
ENV PORT=8000

CMD ["bonscompte-backend"]
