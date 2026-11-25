# syntax=docker/dockerfile:1
# The whole app as one small container image: the game server and the frontend it hands out.
# Build it from the repo root: docker build -t rustyparty .

# Stage 1: build the frontend, with the same Node version as development
FROM node:24.15-trixie-slim AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY frontend ./
RUN npm run build

# Stage 2: compile the server, with the same Rust version as development
FROM rust:1.99-slim-trixie AS backend
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/data ./data
# The caches keep downloaded crates and compiled dependencies between builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked && cp target/release/backend /backend

# Stage 3: run. Only the C library the server needs: no shell, no package manager
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=backend /backend /backend
COPY --from=frontend /app/dist /frontend
# The server hands out these files, and index.html for any other path
ENV FRONTEND_DIR=/frontend
# Quieter logs than in development, without every request. The host can override it
ENV RUST_LOG=backend=info,tower_http=info
# The server listens on PORT, or 3000 when the host doesn't set it
EXPOSE 3000
USER nonroot
ENTRYPOINT ["/backend"]
