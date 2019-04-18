#!/usr/bin/env bash
docker pull clux/muslrust
docker run -v $PWD:/volume -v cargo-cache:/root/.cargo/registry --rm -t clux/muslrust cargo build --release
docker build -t touplitoui/wanaplay-booker-bot .
docker push touplitoui/wanaplay-booker-bot