#!/usr/bin/env bash
# Fail if any ELF needs a glibc newer than the given version (default 2.35,
# Ubuntu 22.04). Checks objdump -T and strings so a too-new symbol cannot hide.
set -euo pipefail

max=${1:-2.35}
shift || true

if ! command -v objdump >/dev/null; then
  echo "objdump is required (install binutils)" >&2
  exit 1
fi

files=("$@")
if [[ ${#files[@]} -eq 0 ]]; then
  files=()
  for candidate in target/release/pageform AppDir/usr/bin/pageform dist/PageForm-x86_64.AppImage; do
    if [[ -f "$candidate" ]]; then
      files+=("$candidate")
    fi
  done
  if [[ -d AppDir ]]; then
    while IFS= read -r -d '' found; do
      files+=("$found")
    done < <(find AppDir -type f \( -name '*.so' -o -name '*.so.*' \) -print0)
  fi
fi

if [[ ${#files[@]} -eq 0 ]]; then
  echo "no binaries to check" >&2
  exit 1
fi

too_new() {
  # True when $1 is a version greater than $max.
  local ver=$1
  [[ "$(printf '%s\n%s\n' "$max" "$ver" | sort -V | tail -1)" == "$ver" && "$ver" != "$max" ]]
}

failed=0
for file in "${files[@]}"; do
  [[ -f "$file" ]] || continue
  if ! objdump -T "$file" >/tmp/pageform-objdump.txt 2>/dev/null; then
    # The AppImage container is not a normal ELF symbol table. Skip it.
    continue
  fi
  mapfile -t versions < <(
    {
      grep -oE 'GLIBC_[0-9]+(\.[0-9]+)*' /tmp/pageform-objdump.txt || true
      strings "$file" | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)*' || true
    } | sed 's/^GLIBC_//' | sort -uV
  )
  if [[ ${#versions[@]} -eq 0 ]]; then
    echo "$file: no GLIBC symbols"
    continue
  fi
  newest=${versions[-1]}
  echo "$file: max GLIBC_${newest}"
  if too_new "$newest"; then
    echo "error: $file requires GLIBC_${newest}; maximum allowed is GLIBC_${max}" >&2
    failed=1
  fi
done

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi
echo "glibc requirement is within GLIBC_${max}"
