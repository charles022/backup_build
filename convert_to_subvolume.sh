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

require_rsync_support() {
    local help
    help="$(rsync --help 2>/dev/null || true)"
    if [ -z "$help" ]; then
        echo "Unable to read rsync help output" >&2
        exit 1
    fi
    for flag in --archive --hard-links --acls --xattrs --clone-dest=; do
        if ! printf '%s' "$help" | grep -q -- "$flag"; then
            echo "rsync does not support required flag: $flag" >&2
            exit 1
        fi
    done
}

if [ "${1-}" = "" ] || [ "${2-}" != "" ]; then
    usage
    exit 1
fi

require_cmd rsync
require_cmd findmnt
require_cmd btrfs
require_cmd lsof
require_cmd mktemp
require_cmd cp
require_cmd rm
require_cmd mv
require_rsync_support

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

tmpdir="$(sudo mktemp -d -p "$parent")"
src_dir="${tmpdir}/src"
basis_dir="${tmpdir}/basis"
dest_dir="${tmpdir}/dest"
sudo mkdir -p "$src_dir" "$basis_dir" "$dest_dir"
sudo sh -c "printf 'reflink-test' > '$src_dir/file'"
sudo cp -a "$src_dir"/. "$basis_dir"/
if ! sudo rsync -a --clone-dest="$basis_dir" "$src_dir"/ "$dest_dir"/ >/dev/null 2>&1; then
    sudo rm -rf "$tmpdir"
    echo "Reflink test failed on filesystem: $parent" >&2
    exit 1
fi
sudo rm -rf "$tmpdir"

sudo mv "$target" "$backup"
sudo btrfs subvolume create "$target" >/dev/null
sudo rsync -aHAX --clone-dest="$backup" "$backup"/ "$target"/
sudo rm -rf "$backup"
