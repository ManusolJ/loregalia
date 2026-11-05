#!/bin/sh
set -eu

apt_retry() {
  attempt=1
  until apt-get -o DPkg::Lock::Timeout=120 "$@"; do
    if [ "$attempt" -ge 10 ]; then
      echo "apt-get failed after $attempt attempts: $*" >&2
      return 1
    fi
    attempt=$((attempt + 1))
    sleep 15
  done
}

apt_retry update
apt_retry install -y \
  gcc g++ clang cmake pkg-config \
  libfontconfig-dev libwayland-dev libwebkit2gtk-4.1-dev \
  libxkbcommon-x11-dev libx11-xcb-dev \
  libssl-dev libzstd-dev libasound2-dev \
  vulkan-validationlayers libvulkan1
