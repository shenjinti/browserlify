#!/bin/sh
set -e

if [ -f ~/.config/fontconfig/fonts.conf ]; then
  fc-cache -fv
fi

/app/browserlify $@

