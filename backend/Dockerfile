# Build stage
FROM rust:bookworm AS builder

WORKDIR /app

# Install SQLite dev libraries
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Resilient crates.io downloads: HTTP/1.1 avoids the "HTTP2 framing layer
# (Connection reset by peer)" errors seen intermittently on CI runners, and
# retry transient network failures instead of failing the whole build.
ENV CARGO_NET_RETRY=10
ENV CARGO_HTTP_MULTIPLEXING=false

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source files to cache dependencies
RUN mkdir -p src/bin && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/admin.rs && \
    echo "fn main() {}" > src/bin/recalculate_contributions.rs && \
    echo "" > src/lib.rs
RUN cargo build --release && rm -rf src

# Copy source and rebuild
COPY . .
RUN touch src/lib.rs src/main.rs src/bin/admin.rs src/bin/recalculate_contributions.rs && cargo build --release

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

# Copy binaries (server and CLI utilities)
COPY --from=builder /app/target/release/bonscompte-backend /usr/local/bin/
COPY --from=builder /app/target/release/bonscompte-admin /usr/local/bin/
RUN ln -s /usr/local/bin/bonscompte-admin /usr/local/bin/admin
COPY --from=builder /app/target/release/recalculate-contributions /usr/local/bin/
COPY --from=builder /app/migrations ./migrations

# Create data directory for SQLite
RUN mkdir -p /data && chown appuser:appuser /data

USER appuser

EXPOSE 8000

ENV DATABASE_URL=sqlite:///data/bonscompte.db
ENV HOST=0.0.0.0
ENV PORT=8000

CMD ["bonscompte-backend"]
