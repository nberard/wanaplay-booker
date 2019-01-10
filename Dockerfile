FROM ekidd/rust-musl-builder as builder
COPY Cargo.toml Cargo.toml
COPY src src
RUN ["cargo", "build" ,"--release"]

FROM alpine
RUN apk update && apk add bash tzdata
RUN apk add tzdata
RUN cp /usr/share/zoneinfo/Europe/Paris  /etc/localtime
RUN echo "Europe/Paris" >  /etc/timezone
RUN apk del tzdata
WORKDIR /bin
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/wanaplay-booker .