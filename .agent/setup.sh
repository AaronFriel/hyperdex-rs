#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
ACTIONLINT_VERSION="${ACTIONLINT_VERSION:-1.7.11}"
GITLEAKS_VERSION="${GITLEAKS_VERSION:-8.30.1}"
NEXTTEST_VERSION="${NEXTTEST_VERSION:-0.9.101}"
PREK_VERSION="${PREK_VERSION:-0.3.8}"
UV_VERSION="${UV_VERSION:-0.6.6}"
ZIZMOR_VERSION="${ZIZMOR_VERSION:-1.23.1}"
UV_INSTALL_DIR="${UV_INSTALL_DIR:-$HOME/.local/bin}"
LOCAL_BIN="${LOCAL_BIN:-$HOME/.local/bin}"
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
}

install_uv() {
  log "installing uv"
  UV_VERSION="$UV_VERSION" UV_INSTALL_DIR="$UV_INSTALL_DIR" scripts/install-uv.sh
  export PATH="$UV_INSTALL_DIR:$PATH"
}

install_uv_tools() {
  export PATH="$UV_INSTALL_DIR:$LOCAL_BIN:$HOME/.cargo/bin:$PATH"

  log "installing prek"
  PREK_VERSION="$PREK_VERSION" LOCAL_BIN="$LOCAL_BIN" scripts/install-prek.sh

  log "installing zizmor"
  uv tool install --force "zizmor==${ZIZMOR_VERSION}"
}

install_repo_tools() {
  log "installing actionlint"
  ACTIONLINT_VERSION="$ACTIONLINT_VERSION" XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}" scripts/install-actionlint.sh >/dev/null

  log "installing gitleaks"
  GITLEAKS_VERSION="$GITLEAKS_VERSION" LOCAL_BIN="$LOCAL_BIN" scripts/install-gitleaks.sh

  log "installing cargo-nextest"
  NEXTTEST_VERSION="$NEXTTEST_VERSION" LOCAL_BIN="$LOCAL_BIN" scripts/install-cargo-nextest.sh
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
gitleaks_version=${GITLEAKS_VERSION}
nextest_version=${NEXTTEST_VERSION}
prek_version=${PREK_VERSION}
uv_version=${UV_VERSION}
zizmor_version=${ZIZMOR_VERSION}
EOF
}

cd "$REPO_ROOT"

install_apt_packages
install_rust_tooling
install_uv
install_uv_tools
install_repo_tools
install_prek_hooks
write_marker

log "setup complete"
echo "PATH additions expected by this repo:"
echo "  $HOME/.cargo/bin"
echo "  $UV_INSTALL_DIR"
echo "  $LOCAL_BIN"
