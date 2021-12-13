FROM rust:1.57.0
RUN apt-get update && apt-get install -y make

# Copy SDK code into the container
RUN mkdir -p $HOME/capi
COPY . $HOME/capi
WORKDIR $HOME/capi

# Run integration tests
# CMD ["/bin/bash", "-c", "make unit && make integration"]
# CMD ["/bin/bash", "-c", "cargo test --workspace"]
