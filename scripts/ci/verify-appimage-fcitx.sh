#!/usr/bin/env bash

set -euo pipefail

target="${1:?usage: verify-appimage-fcitx.sh <rust-target>}"
bundle_dir="target/${target}/release/bundle/appimage"

mapfile -t appimages < <(find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' -print | sort)
if [[ ${#appimages[@]} -ne 1 ]]; then
  echo "Expected exactly one AppImage in ${bundle_dir}, found ${#appimages[@]}" >&2
  exit 1
fi

appimage="$(realpath "${appimages[0]}")"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

(
  cd "$work_dir"
  chmod +x "$appimage"
  "$appimage" --appimage-extract >/dev/null

  module_path="$(find squashfs-root -name 'im-fcitx5.so' -print -quit)"
  if [[ -z "$module_path" ]]; then
    echo "AppImage does not contain the fcitx5 GTK3 input method module" >&2
    exit 1
  fi

  cache_path=""
  while IFS= read -r candidate; do
    if grep -qi 'fcitx' "$candidate"; then
      cache_path="$candidate"
      break
    fi
  done < <(find squashfs-root -name 'immodules.cache' -type f -print)

  if [[ -z "$cache_path" ]]; then
    echo "AppImage GTK immodules.cache does not register fcitx5" >&2
    exit 1
  fi

  echo "Verified AppImage fcitx5 GTK module: module=${module_path}, cache=${cache_path}"
)
