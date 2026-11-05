#!/bin/sh
set -eu

ci_packages="gcc g++ make pkg-config libc6-dev libfontconfig-dev libxkbcommon-x11-dev ca-certificates"
sandbox_packages="$ci_packages libwayland-dev libx11-xcb-dev vulkan-validationlayers libvulkan1 mesa-vulkan-drivers"

case "${1:-sandbox}" in
  ci) packages=$ci_packages ;;
  sandbox) packages=$sandbox_packages ;;
  *)
    echo "usage: linux-packages.sh [ci|sandbox]" >&2
    exit 2
    ;;
esac

export DEBIAN_FRONTEND=noninteractive

run_apt() {
  apt-get \
    -o DPkg::Lock::Timeout=120 \
    -o Acquire::Retries=3 \
    -o Acquire::http::Timeout=30 \
    -o Acquire::https::Timeout=30 \
    "$@"
}

install_packages() {
  run_apt install -y --no-install-recommends $packages
}

if ! install_packages; then
  run_apt update
  install_packages
fi
