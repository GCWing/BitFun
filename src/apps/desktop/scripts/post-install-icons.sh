#!/bin/bash
# post-install script: install multi-size icons to hicolor theme
# Tauri only installs 1024x1024; this adds smaller sizes for taskbar/dock/alt-tab

ICON_SRC="/usr/lib/BitFun/share/icons"
ICON_DST_BASE="/usr/share/icons/hicolor"

# Try multiple possible source locations
for src_dir in \
  "/usr/lib/BitFun/share/icons" \
  "/opt/bitfun/share/icons" \
  "/usr/share/bitfun/icons"; do
  if [ -d "$src_dir/hicolor" ]; then
    ICON_SRC="$src_dir/hicolor"
    break
  fi
done

if [ ! -d "$ICON_SRC" ]; then
  exit 0
fi

# Copy all icon sizes to system hicolor theme
cp -rn "$ICON_SRC"/* "$ICON_DST_BASE/" 2>/dev/null || true

# Update icon cache
if command -v gtk-update-icon-cache &>/dev/null; then
  gtk-update-icon-cache -f "$ICON_DST_BASE" 2>/dev/null || true
fi

exit 0
