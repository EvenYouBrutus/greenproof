# GreenProof backend deployment image
# Builds Rust + Circom + ZK artifacts inside the image so the main branch
# does not need generated circuit binaries committed.

FROM rust:1.88-bookworm AS build

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends nodejs npm pkg-config libssl-dev ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

# Circom is a Rust binary. The official docs install it with cargo.
RUN cargo install --git https://github.com/iden3/circom --tag v2.2.3 circom

WORKDIR /app
COPY . .

# Generate the circuit artifacts required by the backend verifier.
RUN bash scripts/setup.sh

# Build the Rust API.
RUN cargo build --release --manifest-path backend/Cargo.toml

FROM node:22-bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build /app/backend/target/release/greenproof-backend /app/greenproof-backend
COPY --from=build /app/scripts /app/scripts
COPY --from=build /app/circuits /app/circuits

WORKDIR /app/scripts
RUN npm install --omit=dev --no-audit --no-fund

WORKDIR /app

ENV GREENPROOF_BACKEND_ADDR=0.0.0.0:8080
ENV GREENPROOF_SCRIPTS_DIR=/app/scripts
ENV GREENPROOF_VKEY_PATH=/app/circuits/build/verification_key.json

EXPOSE 8080

CMD ["/app/greenproof-backend"]
