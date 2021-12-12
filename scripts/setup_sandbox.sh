#!/usr/bin/env bash

set -e

rm -rf sandbox
git clone --single-branch --branch master https://github.com/ivanschuetz/sandbox.git sandbox

cd sandbox


echo "PWD::"
pwd


# #build test environment
# docker build -t rust-sdk-testing -f tests/docker/Dockerfile "$(pwd)"

# # Start test harness environment
# ./test-harness/scripts/up.sh -p

# docker run --network host rust-sdk-testing:latest
