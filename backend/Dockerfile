# syntax=docker/dockerfile:1
# The backend server as a small container image, for hosts that run Docker.
# Build it from this folder: docker build -t rustyparty-backend .

# Stage 1: compile, with the same Rust version as development
FROM rust:1.99-slim-trixie AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data
# The caches keep downloaded crates and compiled dependencies between builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked && cp target/release/backend /backend

# Stage 2: run. Only the C library the server needs: no shell, no package manager
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=build /backend /backend
# Quieter logs than in development, without every request. The host can override it
ENV RUST_LOG=backend=info,tower_http=info
# The server listens on PORT, or 3000 when the host doesn't set it
EXPOSE 3000
USER nonroot
ENTRYPOINT ["/backend"]
