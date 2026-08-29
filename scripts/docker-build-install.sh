#!/usr/bin/env bash
# Build grok in Docker and replace the managed install at ~/.grok/bin/grok.
#
# Usage:
#   scripts/docker-build-install.sh              # build + install
#   scripts/docker-build-install.sh --build-only
#   scripts/docker-build-install.sh --install-only

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
IMAGE="grok-local-build"
MODE="all"

for arg in "$@"; do
  case "$arg" in
    --build-only) MODE="build" ;;
    --install-only) MODE="install" ;;
    -h|--help)
      sed -n '2,8p' "$0"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

build() {
  mkdir -p "$DIST"
  echo "Building grok in Docker (this takes a while on first run)..."
  docker build \
    --target export \
    --output "type=local,dest=$DIST" \
    -f "$ROOT/docker/Dockerfile" \
    "$ROOT"
  chmod +x "$DIST/grok"
  echo "Built: $DIST/grok"
  "$DIST/grok" --version
}

install_bin() {
  local src="$DIST/grok"
  if [[ ! -x "$src" ]]; then
    echo "missing $src — run without --install-only first" >&2
    exit 1
  fi

  local grok_home="${GROK_HOME:-$HOME/.grok}"
  local downloads="$grok_home/downloads"
  local dest="$downloads/grok-local-linux-x86_64"
  local link="$grok_home/bin/grok"

  mkdir -p "$downloads" "$grok_home/bin"
  cp -f "$src" "$dest"
  chmod +x "$dest"

  if [[ -L "$link" || -e "$link" ]]; then
    echo "Previous grok: $(readlink -f "$link" 2>/dev/null || echo "$link")"
  fi
  ln -sfn "../downloads/grok-local-linux-x86_64" "$link"

  # Keep ~/.local/bin/grok pointing at the managed symlink when it already does.
  local local_bin="${HOME}/.local/bin/grok"
  if [[ ! -e "$local_bin" ]]; then
    mkdir -p "${HOME}/.local/bin"
    ln -sfn "$link" "$local_bin"
  fi

  # Official auto-update would retarget ~/.grok/bin/grok at a downloaded
  # release. Force it off so this source build stays in place.
  local cfg="$grok_home/config.toml"
  if [[ -f "$cfg" ]] && grep -qE '^[[:space:]]*auto_update[[:space:]]*=' "$cfg"; then
    sed -i.bak-local 's/^[[:space:]]*auto_update[[:space:]]*=.*/auto_update = false/' "$cfg"
    echo "Set auto_update = false in $cfg (backup: ${cfg}.bak-local)"
  elif [[ -f "$cfg" ]]; then
    if grep -q '^\[cli\]' "$cfg"; then
      sed -i.bak-local '/^\[cli\]/a auto_update = false' "$cfg"
    else
      printf '\n[cli]\nauto_update = false\n' >>"$cfg"
    fi
    echo "Set auto_update = false in $cfg"
  fi

  hash -r 2>/dev/null || true
  echo "Installed: $link -> $dest"
  echo "which grok: $(command -v grok || echo "$local_bin")"
  grok --version
}

case "$MODE" in
  build) build ;;
  install) install_bin ;;
  all)
    build
    install_bin
    ;;
esac
