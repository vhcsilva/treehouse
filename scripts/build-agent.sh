#!/usr/bin/env bash
# Compila o treehouse-agent (Linux) e coloca o binário onde o instalador do app o
# procura. Rode DENTRO do WSL, a partir da raiz do repositório.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
# Compilar dentro de /mnt/c é lento: o cache de build fica no Linux.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/treehouse-target}"

target=""
if rustup target list --installed 2>/dev/null | grep -q x86_64-unknown-linux-musl; then
  target="x86_64-unknown-linux-musl" # binário estático: roda em qualquer distro
fi

if [[ -n "$target" ]]; then
  cargo build --release -p treehouse-agent --target "$target" --manifest-path "$root/Cargo.toml"
  bin="$CARGO_TARGET_DIR/$target/release/treehouse-agent"
else
  echo "aviso: alvo musl não instalado; gerando binário para esta distro (rustup target add x86_64-unknown-linux-musl para um estático)"
  cargo build --release -p treehouse-agent --manifest-path "$root/Cargo.toml"
  bin="$CARGO_TARGET_DIR/release/treehouse-agent"
fi

dest="$root/app/src-tauri/resources/agent/treehouse-agent"
cp "$bin" "$dest"
chmod +x "$dest"
echo "ok: $("$dest" version) → $dest"
