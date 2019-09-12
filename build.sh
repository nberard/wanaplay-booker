#!/usr/bin/env bash
#docker run \
#    -v "$(pwd):/home/rust/src" \
#    -v "$CARGO_PATH/git:/home/rust/.cargo/git" \
#    -v "$CARGO_PATH/registry:/home/rust/.cargo/registry" \
#    -v $(pwd)/target:/home/rust/src/target \
#    --rm -it ekidd/rust-musl-builder:nightly
# inside container
# sudo chown -R rust:rust /home/rust/.cargo/git /home/rust/.cargo/registry /home/rust/src/target
# cargo build --release
# outside container
# sudo chown $(whoami):$(whoami) target -R
docker build -t touplitoui/wanaplay-booker-bot .
docker push touplitoui/wanaplay-booker-bot
docker build -f Dockerfile.api -t touplitoui/wanaplay-booker-api .
docker push touplitoui/wanaplay-booker-api
pushd wanabot
docker build -f Dockerfile -t touplitoui/wanaplay-booker-chatbot .
docker push touplitoui/wanaplay-booker-chatbot
popd
