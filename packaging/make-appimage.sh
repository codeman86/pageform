#!/usr/bin/env bash
# Build a portable PageForm AppImage from a release binary.
# Run from the repository root. Expects target/release/pageform,
# or builds it when PAGEFORM_BUILD=1.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

if [[ "${PAGEFORM_BUILD:-0}" == 1 || ! -x target/release/pageform ]]; then
  cargo build --release -p pageform
fi

bin=target/release/pageform
if [[ ! -f "$bin" ]]; then
  echo "missing $bin" >&2
  exit 1
fi

echo "Shared libraries before bundling:"
ldd "$bin" || true

rm -rf AppDir
mkdir -p AppDir/usr/bin
install -m 755 "$bin" AppDir/usr/bin/pageform
install -D -m 644 packaging/pageform.desktop AppDir/usr/share/applications/pageform.desktop
install -D -m 644 packaging/pageform.png AppDir/usr/share/icons/hicolor/256x256/apps/pageform.png

tools=${PAGEFORM_TOOLS:-"$root/.appimage-tools"}
mkdir -p "$tools"
linuxdeploy=$tools/linuxdeploy-x86_64.AppImage
appimagetool=$tools/appimagetool-x86_64.AppImage
if [[ ! -x "$linuxdeploy" ]]; then
  curl -fsSL -o "$linuxdeploy" \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  chmod +x "$linuxdeploy"
fi
if [[ ! -x "$appimagetool" ]]; then
  curl -fsSL -o "$appimagetool" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$appimagetool"
fi

export PATH="$tools:$PATH"
export APPIMAGE_EXTRACT_AND_RUN=1
# Keep the host GPU driver. Bundling Ubuntu's libGL/libEGL/libvulkan breaks Arch.
export LINUXDEPLOY_EXCLUDED_LIBRARIES="${LINUXDEPLOY_EXCLUDED_LIBRARIES:-libGL.so.1:libEGL.so.1:libvulkan.so.1}"

library_args=()
for name in libxkbcommon.so.0 libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1; do
  found=$(ldconfig -p | awk -v lib="$name" '$1 == lib { print $NF; exit }')
  if [[ -n "${found:-}" && -f "$found" ]]; then
    library_args+=(--library "$found")
  fi
done

rm -f PageForm-*.AppImage dist/PageForm-x86_64.AppImage
"$linuxdeploy" --appdir AppDir \
  --executable AppDir/usr/bin/pageform \
  --desktop-file AppDir/usr/share/applications/pageform.desktop \
  --icon-file packaging/pageform.png \
  "${library_args[@]}" \
  --output appimage

produced=$(echo PageForm-*.AppImage)
if [[ ! -f "$produced" ]]; then
  echo "linuxdeploy did not write an AppImage" >&2
  exit 1
fi

mkdir -p dist
install -m 755 "$produced" dist/PageForm-x86_64.AppImage
rm -f "$produced"

bundled=$(find AppDir -name '*.so*' | wc -l)
echo "Bundled shared objects: $bundled"
if [[ "$bundled" -lt 1 ]]; then
  echo "AppDir has no shared libraries; the AppImage would not be portable." >&2
  exit 1
fi

size=$(stat -c%s dist/PageForm-x86_64.AppImage)
echo "dist/PageForm-x86_64.AppImage size: $size"
if [[ "$size" -lt 1000000 ]]; then
  echo "AppImage is unexpectedly small" >&2
  exit 1
fi

file dist/PageForm-x86_64.AppImage
# Smoke the CLI without a window. --help exits before the GUI.
dist/PageForm-x86_64.AppImage --appimage-extract-and-run --help >/tmp/pageform-appimage-help.txt
grep -q "PageForm" /tmp/pageform-appimage-help.txt
echo "AppImage --help ok"
