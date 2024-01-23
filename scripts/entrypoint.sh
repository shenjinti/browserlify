#!/bin/sh
set -e

if [ -f /app/fonts/local.conf ]; then
  mkdir -p ~/.config/fontconfig/ && ln -s /app/fonts/local.conf ~/.config/fontconfig/local.conf
  fc-cache -fv
fi

/app/browserlify $@

