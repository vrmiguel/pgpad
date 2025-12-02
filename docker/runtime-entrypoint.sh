#!/bin/sh
set -e

# Find AppImage inside bundled outputs
APPIMAGE=$(ls /opt/pgpad/bundle/appimage/*.AppImage 2>/dev/null | head -n 1 || true)

if [ -z "$APPIMAGE" ]; then
  echo "No AppImage found under /opt/pgpad/bundle/appimage" >&2
  echo "Contents:" >&2
  ls -lah /opt/pgpad/bundle || true
  exit 1
fi

# Ensure data dir exists
mkdir -p "${PGPAD_DATA_DIR:-$HOME/.local/share/pgpad}"

# Honor X11/Wayland forwarding from host via environment
exec "$APPIMAGE"

