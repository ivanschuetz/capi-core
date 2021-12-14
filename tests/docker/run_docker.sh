#!/usr/bin/env bash

set -e

# reset test harness
rm -rf test-harness
rm -rf tests/features
# fork with modified features, as cucumber-rs doesn't understand some syntax:
# https://github.com/cucumber-rs/cucumber/issues/174
# https://github.com/cucumber-rs/cucumber/issues/175
# git clone --single-branch --branch master https://github.com/algorand/algorand-sdk-testing.git test-harness
git clone --single-branch --branch master https://github.com/ivanschuetz/algorand-sdk-testing.git test-harness

RUST_IMAGE=rust:1.57.0
echo "Building docker image from base \"$RUST_IMAGE\""

#build test environment
docker build -t rust-sdk-testing -f tests/docker/Dockerfile "$(pwd)"

# Start test harness environment
./test-harness/scripts/up.sh -p

docker run --network host rust-sdk-testing:latest
