FROM debian:bookworm
RUN apt-get update && apt-get install -y ca-certificates tzdata
ENV DEBIAN_FRONTEND noninteractive
ENV LANG C.UTF-8
RUN apt-get install -y --no-install-recommends tzdata \
    libasound2 \
    libatk-bridge2.0-0 \
    libatk1.0-0 \
    libatspi2.0-0 \
    libcairo2 \
    libcups2 \
    libcurl3-nss \
    libdbus-1-3 \
    libdrm2 \
    libgbm1 \
    libglib2.0-0 \
    libgtk-4-1 \
    libpango-1.0-0 \
    libx11-6 \
    libxcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxext6 \
    libxfixes3 \
    libxrandr2 \
    wget \
    xdg-utils 

ADD google-chrome-stable_current_amd64.deb /tmp/
RUN dpkg -i /tmp/google-chrome-stable_current_amd64.deb
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
COPY scripts/*.sh /app/

RUN mkdir -p /app/Downloads
ENV CHROME_BIN=/usr/bin/google-chrome \
    CHROME_PATH=/opt/google/chrome/

ENTRYPOINT [ "/app/entrypoint.sh"]