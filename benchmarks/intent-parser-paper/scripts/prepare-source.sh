#!/usr/bin/env sh
set -eu

revision="5a110e78a8440921d7d4302769bc049180f9d2bf"
artifact_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
vendor_root="$artifact_root/vendor"
source_root="$vendor_root/Auwgent-$revision"
patch_path="$artifact_root/patches/parser-hardening.patch"

if [ ! -d "$source_root" ]; then
  mkdir -p "$vendor_root"
  archive="$vendor_root/Auwgent-$revision.tar.gz"
  url="https://codeload.github.com/snrraptopack/Auwgent/tar.gz/$revision"

  printf '%s\n' "Downloading immutable Auwgent source revision $revision"
  curl -L --fail --silent --show-error "$url" -o "$archive"
  tar -xzf "$archive" -C "$vendor_root"
  rm -f "$archive"
fi

if [ ! -d "$source_root" ]; then
  printf '%s\n' "Archive did not produce expected source directory: $source_root" >&2
  exit 1
fi

old_ceiling=${GIT_CEILING_DIRECTORIES-}
export GIT_CEILING_DIRECTORIES="$artifact_root"
cd "$source_root"

if git apply --reverse --check "$patch_path" 2>/dev/null; then
  printf '%s\n' "Pinned source and parser-hardening patch already prepared: $source_root"
  exit 0
fi

if ! git apply --check "$patch_path"; then
  printf '%s\n' "Pinned source is neither pristine nor patched; remove vendor/Auwgent-$revision and retry" >&2
  exit 1
fi
git apply "$patch_path"

if [ -n "$old_ceiling" ]; then
  export GIT_CEILING_DIRECTORIES="$old_ceiling"
else
  unset GIT_CEILING_DIRECTORIES
fi

printf '%s\n' "Prepared pinned source plus parser-hardening patch: $source_root"
