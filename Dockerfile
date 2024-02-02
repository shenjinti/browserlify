FROM rust:bookworm as rust-builder
RUN mkdir /build
ADD . /build/
WORKDIR /build
RUN cargo build --release

FROM node:bookworm as node-builder
RUN mkdir /build
ADD web /build/
WORKDIR /build
RUN npm config set registry http://registry.npmmirror.com
RUN yarn && yarn build

FROM debian:bookworm
ENV DEBIAN_FRONTEND noninteractive
ENV LANG C.UTF-8
RUN apt-get update -y && apt-get install -y ca-certificates tzdata chromium wget \
    fonts-liberation  \
    fonts-ipafont-gothic \
    fonts-noto-cjk \
    fonts-roboto \
    fonts-noto-color-emoji \
    fonts-noto-color-emoji \
    fonts-freefont-ttf \
    fonts-thai-tlwg \
    fonts-indic \
    fontconfig x11vnc xvfb scrot

RUN rm -rf /var/cache/* \
    && apt-get -qq clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* \
    && rm -Rf /var/cache/apt

# Add Chrome as a user
RUN mkdir -p /app \
    && adduser chrome --home /app\
    && chown -R chrome:chrome /app

# Run Chrome as non-privileged
USER chrome
WORKDIR /app

RUN mkdir -p /app/Downloads && mkdir -p /tmp/browserlify/
ENV CHROME_BIN=/usr/bin/google-chrome \
    CHROME_PATH=/opt/google/chrome/

COPY scripts/*.sh /app/
COPY --from=rust-builder /build/target/release/browserlify /app/
COPY --from=node-builder /build/dist /app/dist

ENTRYPOINT [ "/app/entrypoint.sh"]