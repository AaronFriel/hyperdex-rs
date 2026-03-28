#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ACTIONLINT_VERSION="${ACTIONLINT_VERSION:-1.7.11}"
PREK_VERSION="${PREK_VERSION:-0.3.8}"
ZIZMOR_VERSION="${ZIZMOR_VERSION:-1.23.1}"
UV_INSTALL_DIR="${UV_INSTALL_DIR:-$HOME/.local/bin}"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"
ACTIONLINT_BIN="${LOCAL_BIN}/actionlint"
MARKER_PATH="${REPO_ROOT}/.agent/.setup_done"

log() {
  printf '==> %s\n' "$*"
}

run_as_root() {
  if command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    "$@"
  fi
}

install_apt_packages() {
  if ! command -v apt-get >/dev/null 2>&1; then
    return 0
  fi

  log "installing system packages"
  run_as_root apt-get update
  run_as_root apt-get install -y \
    build-essential \
    clang \
    cmake \
    curl \
    git \
    libclang-dev \
    pkg-config \
    protobuf-compiler
}

install_rust_tooling() {
  if ! command -v rustup >/dev/null 2>&1; then
    log "installing rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  fi

  log "installing rust toolchains and components"
  rustup toolchain install stable
  rustup toolchain install nightly
  rustup component add clippy rustfmt --toolchain stable
  rustup component add rustfmt --toolchain nightly

  if ! command -v cargo-binstall >/dev/null 2>&1; then
    log "installing cargo-binstall"
    curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/v1.14.3/install-from-binstall-release.sh \
      | BINSTALL_VERSION=v1.14.3 bash
    export PATH="$HOME/.cargo/bin:$PATH"
  fi

  log "installing cargo-nextest"
  cargo binstall -y cargo-nextest
}

install_uv() {
  if command -v uv >/dev/null 2>&1; then
    return 0
  fi

  log "installing uv"
  curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR="$UV_INSTALL_DIR" sh
  export PATH="$UV_INSTALL_DIR:$PATH"
}

install_uv_tools() {
  export PATH="$UV_INSTALL_DIR:$LOCAL_BIN:$HOME/.cargo/bin:$PATH"

  log "installing prek"
  uv tool install --force "prek==${PREK_VERSION}"

  log "installing zizmor"
  uv tool install --force "zizmor==${ZIZMOR_VERSION}"
}

install_actionlint() {
  mkdir -p "$LOCAL_BIN"

  if [[ -x "$ACTIONLINT_BIN" ]] && "$ACTIONLINT_BIN" -version 2>/dev/null | grep -q "$ACTIONLINT_VERSION"; then
    return 0
  fi

  log "installing actionlint"
  local archive
  archive="$(mktemp -t actionlint.XXXXXX.tar.gz)"
  trap 'rm -f "$archive"' RETURN
  curl -L --fail --silent --show-error \
    "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_linux_amd64.tar.gz" \
    -o "$archive"
  tar -xzf "$archive" -C "$LOCAL_BIN" actionlint
  chmod +x "$ACTIONLINT_BIN"
}

install_prek_hooks() {
  export PATH="$UV_INSTALL_DIR:$LOCAL_BIN:$HOME/.cargo/bin:$PATH"
  log "installing prek git hooks"
  prek run --all-files --dry-run -c .pre-commit-config.yaml >/dev/null
  prek install -c .pre-commit-config.yaml --overwrite --hook-type pre-commit
}

write_marker() {
  cat >"$MARKER_PATH" <<EOF
setup_completed_at=$(date -Iseconds)
actionlint_version=${ACTIONLINT_VERSION}
prek_version=${PREK_VERSION}
zizmor_version=${ZIZMOR_VERSION}
EOF
}

cd "$REPO_ROOT"

install_apt_packages
install_rust_tooling
install_uv
install_uv_tools
install_actionlint
install_prek_hooks
write_marker

log "setup complete"
echo "PATH additions expected by this repo:"
echo "  $HOME/.cargo/bin"
echo "  $UV_INSTALL_DIR"
echo "  $LOCAL_BIN"
