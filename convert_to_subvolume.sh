#!/bin/bash
set -euo pipefail

usage() {
    echo "Usage: $0 /path/to/dir" >&2
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

if [ "${1-}" = "" ] || [ "${2-}" != "" ]; then
    usage
    exit 1
fi

require_cmd findmnt
require_cmd btrfs
require_cmd lsof
require_cmd mktemp
require_cmd cp
require_cmd rm
require_cmd mv

target="$1"
if [ "$target" = "/" ]; then
    echo "Refusing to convert / into a subvolume." >&2
    exit 1
fi

if ! sudo test -d "$target"; then
    echo "Path is not a directory: $target" >&2
    exit 1
fi

fstype=$(sudo findmnt -n -o FSTYPE -T "$target" 2>/dev/null || true)
if [ -z "$fstype" ]; then
    fstype=$(sudo findmnt -n -o FSTYPE -T "$(dirname -- "$target")" 2>/dev/null || true)
fi
if [ "$fstype" != "btrfs" ]; then
    if [ -z "$fstype" ]; then
        echo "Unable to determine filesystem type for: $target" >&2
    else
        echo "Target is not on btrfs (found: $fstype): $target" >&2
    fi
    exit 1
fi

if sudo btrfs subvolume show "$target" &>/dev/null; then
    echo "Already a btrfs subvolume: $target"
    exit 0
fi

parent=$(dirname -- "$target")
base=$(basename -- "$target")
backup="${parent}/${base}-old-$(date +%Y%m%d_%H%M%S)"

if sudo lsof +D "$target" >/dev/null 2>&1; then
    echo "Directory appears to be in use; stop writers before converting: $target" >&2
    exit 1
fi

# Verify reflink support with a simple test
tmpdir="$(sudo mktemp -d -p "$parent")"
src_file="${tmpdir}/src"
dest_file="${tmpdir}/dest"
sudo sh -c "printf 'reflink-test' > '$src_file'"
if ! sudo cp --reflink=always "$src_file" "$dest_file" >/dev/null 2>&1; then
    sudo rm -rf "$tmpdir"
    echo "Reflink test failed on filesystem: $parent. 'cp --reflink=always' is required." >&2
    exit 1
fi
sudo rm -rf "$tmpdir"

# Perform conversion
sudo mv "$target" "$backup"
sudo btrfs subvolume create "$target" >/dev/null
sudo cp -a --reflink=always "$backup"/. "$target"/
sudo rm -rf "$backup"