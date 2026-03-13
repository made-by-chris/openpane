#!/bin/sh

set -eu

REPO="made-by-chris/openpane"
API_URL="https://api.github.com/repos/$REPO/releases/latest"
BIN_DIR="${HOME}/.local/bin"
LIB_ROOT="${HOME}/.local/share/openpane"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'openpane installer error: missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar
need_cmd node

VERSION="${OPENPANE_VERSION:-}"

if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "$API_URL" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p' | head -n 1)"
fi

if [ -z "$VERSION" ]; then
  printf 'openpane installer error: unable to determine latest release.\n' >&2
  exit 1
fi

ARCHIVE_URL="https://github.com/$REPO/archive/refs/tags/v$VERSION.tar.gz"
TMP_DIR="$(mktemp -d)"
ARCHIVE_PATH="$TMP_DIR/openpane.tar.gz"
INSTALL_DIR="$LIB_ROOT/$VERSION"

cleanup() {
  rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

mkdir -p "$BIN_DIR" "$LIB_ROOT"

printf 'Downloading openpane v%s...\n' "$VERSION"
curl -fsSL "$ARCHIVE_URL" -o "$ARCHIVE_PATH"

rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"
cp -R "$TMP_DIR/openpane-$VERSION/." "$INSTALL_DIR/"

create_wrapper() {
  name="$1"
  wrapper_path="$BIN_DIR/$name"
  cat > "$wrapper_path" <<EOF
#!/bin/sh
exec node "$INSTALL_DIR/bin/grid.js" "\$@"
EOF
  chmod +x "$wrapper_path"
}

create_wrapper "openpane"
create_wrapper "grid"
create_wrapper "codegrid"

printf '\nInstalled openpane to %s\n' "$INSTALL_DIR"
printf 'Command shims created in %s\n' "$BIN_DIR"

case ":$PATH:" in
  *":$BIN_DIR:"*)
    printf 'You can run: openpane 2 2 claude\n'
    ;;
  *)
    printf 'Add this to your shell profile, then restart your shell:\n'
    printf '  export PATH="%s:$PATH"\n' "$BIN_DIR"
    ;;
esac
