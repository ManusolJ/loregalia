#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

sudo sh "$project_root/scripts/linux-packages.sh"

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi

persistent=/etc/sandbox-persistent.sh
sudo tee -a "$persistent" >/dev/null <<'EOF'
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$HOME/loregalia-target"
EOF

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$HOME/loregalia-target"

cd "$project_root"
rustup toolchain install
git config core.hooksPath .githooks

echo "Sandbox ready. Open a new shell, then run: cargo xtask gate"
