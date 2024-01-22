#!/bin/sh
set -e

if [ -f /app/fonts/local.conf ]; then
  rm /etc/fonts/local.conf && ln -s /app/fonts/local.conf /etc/fonts/local.conf
  fc-cache -fv
fi

/app/bin/browserlify $@

