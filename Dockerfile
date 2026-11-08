# ==============================================================================
# Stage 1: Deterministic Pure-Rust Builder (Zero External Dependencies)
# ==============================================================================
FROM rust:1.82-slim AS builder

WORKDIR /usr/src/garry-math-oss
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

# Verify all unit tests and compile optimized release binary
RUN cargo test --release && \
    cargo build --release --locked

# ==============================================================================
# Stage 2: Minimal Air-Gapped Runtime Container
# ==============================================================================
FROM debian:bookworm-slim

RUN useradd -m -u 10001 garry && \
    mkdir -p /app/garry_workspace/knowledge \
             /app/garry_workspace/scripts \
             /app/garry_workspace/solutions && \
    chown -R garry:garry /app

WORKDIR /app
COPY --from=builder /usr/src/garry-math-oss/target/release/garry-math-oss /usr/local/bin/garry-math-oss

USER garry
VOLUME ["/app/garry_workspace"]

ENTRYPOINT ["/usr/local/bin/garry-math-oss"]