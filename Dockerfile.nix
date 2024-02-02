FROM nixos/nix:latest AS builder

# Copy our source and setup our working dir.
COPY go.mod /tmp/build/go.mod
COPY go.sum /tmp/build/go.sum
COPY flake.nix /tmp/build/flake.nix
COPY flake.lock /tmp/build/flake.lock
COPY gomod2nix.toml /tmp/build/gomod2nix.toml
COPY cmd /tmp/build/cmd/
COPY internal /tmp/build/internal/
COPY pkg /tmp/build/pkg/
WORKDIR /tmp/build

# Build our Nix environment
RUN nix \
    --extra-experimental-features "nix-command flakes" \
    --option filter-syscalls false \
    build

# Copy the Nix store closure into a directory. The Nix store closure is the
# entire set of Nix store values that we need for our build.
RUN mkdir /tmp/nix-store-closure
RUN cp -R $(nix-store -qR result/) /tmp/nix-store-closure

# Final image is based on scratch. We copy a bunch of Nix dependencies
# but they're fully self-contained so we don't need Nix anymore.
FROM scratch

WORKDIR /app

# Copy /nix/store
COPY --from=builder /tmp/nix-store-closure /nix/store
COPY --from=builder /tmp/build/result /app
ENTRYPOINT ["/app/bin/q3"]
