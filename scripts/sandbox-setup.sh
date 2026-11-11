#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

startup_apt_checks=0
while pgrep -x apt-get >/dev/null && [ "$startup_apt_checks" -lt 36 ]; do
  startup_apt_checks=$((startup_apt_checks + 1))
  sleep 5
done

sudo sh "$project_root/scripts/linux-packages.sh" sandbox

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi

persistent=/etc/sandbox-persistent.sh
if ! grep -q 'loregalia-target' "$persistent" 2>/dev/null; then
  sudo tee -a "$persistent" >/dev/null <<'EOF'
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$HOME/loregalia-target"
EOF
fi
if ! grep -q 'CARGO_INCREMENTAL' "$persistent" 2>/dev/null; then
  echo 'export CARGO_INCREMENTAL=0' | sudo tee -a "$persistent" >/dev/null
fi

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$HOME/loregalia-target"
export CARGO_INCREMENTAL=0

cd "$project_root"
rustup toolchain install
git config core.hooksPath .githooks
git config --global user.name "Loregalia Agent"
git config --global user.email "agent@loregalia.invalid"

echo "Sandbox ready. Open a new shell, then run: cargo xtask gate"
