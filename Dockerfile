FROM alpine
ARG TAG=release
RUN apk update && apk add bash tzdata
RUN apk add tzdata
RUN cp /usr/share/zoneinfo/Europe/Paris  /etc/localtime
RUN echo "Europe/Paris" >  /etc/timezone
RUN apk del tzdata
WORKDIR /bin
COPY target/x86_64-unknown-linux-musl/${TAG}/wanaplay-booker .
COPY target/x86_64-unknown-linux-musl/${TAG}/wanaplay-reminder .