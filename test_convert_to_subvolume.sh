#!/bin/bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
converter="${here}/convert_to_subvolume.sh"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        fail "Missing required command: $1"
    fi
}

require_cmd sudo
require_cmd findmnt
require_cmd btrfs
require_cmd rsync
require_cmd lsof

require_rsync_support() {
    local help
    help="$(rsync --help 2>/dev/null || true)"
    if [ -z "$help" ]; then
        fail "Unable to read rsync help output"
    fi
    for flag in --archive --hard-links --acls --xattrs --clone-dest=; do
        if ! printf '%s' "$help" | grep -q -- "$flag"; then
            fail "rsync does not support required flag: $flag"
        fi
    done
}

require_rsync_support

if [ ! -x "$converter" ]; then
    fail "Converter not found or not executable: $converter"
fi

base="${TEST_BTRFS_BASE-}"
if [ -z "$base" ]; then
    fstype="$(findmnt -n -o FSTYPE -T "$HOME" 2>/dev/null || true)"
    if [ "$fstype" = "btrfs" ]; then
        base="$HOME"
    else
        fail "Set TEST_BTRFS_BASE to a writable directory on btrfs."
    fi
fi

test_root="$(sudo mktemp -d -p "$base" convert-subvol-test.XXXXXX)"
trap 'sudo rm -rf "$test_root"' EXIT

echo "Using test root: $test_root"

echo "Test: missing args"
if "$converter" >/dev/null 2>&1; then
    fail "Expected failure for missing args"
fi

echo "Test: not a directory"
file_path="$test_root/not-a-dir"
sudo sh -c "printf 'x' > '$file_path'"
if "$converter" "$file_path" >/dev/null 2>&1; then
    fail "Expected failure for non-directory path"
fi

echo "Test: already a subvolume"
subvol_path="$test_root/already-subvol"
sudo btrfs subvolume create "$subvol_path" >/dev/null
out_file="$(mktemp)"
if ! "$converter" "$subvol_path" >"$out_file" 2>&1; then
    echo "Converter output:" >&2
    cat "$out_file" >&2
    rm -f "$out_file"
    fail "Expected clean exit for existing subvolume"
fi
if ! grep -q "Already a btrfs subvolume" "$out_file"; then
    rm -f "$out_file"
    fail "Expected 'Already a btrfs subvolume' message"
fi
rm -f "$out_file"
sudo btrfs subvolume delete "$subvol_path" >/dev/null

echo "Test: directory in use"
busy_dir="$test_root/busy-dir"
sudo mkdir -p "$busy_dir"
sudo sh -c "printf 'busy' > '$busy_dir/busy.txt'"
tail -f "$busy_dir/busy.txt" >/dev/null 2>&1 &
tail_pid=$!
sleep 1
out_file="$(mktemp)"
if "$converter" "$busy_dir" >"$out_file" 2>&1; then
    kill "$tail_pid" >/dev/null 2>&1 || true
    wait "$tail_pid" >/dev/null 2>&1 || true
    rm -f "$out_file"
    fail "Expected failure for in-use directory"
fi
kill "$tail_pid" >/dev/null 2>&1 || true
wait "$tail_pid" >/dev/null 2>&1 || true
if ! grep -q "in use" "$out_file"; then
    cat "$out_file" >&2
    rm -f "$out_file"
    fail "Expected in-use warning message"
fi
rm -f "$out_file"

echo "Test: convert directory"
dir_path="$test_root/data-dir"
sudo mkdir -p "$dir_path/.hidden"
sudo sh -c "printf 'hello' > '$dir_path/file.txt'"
sudo sh -c "printf 'dot' > '$dir_path/.hidden/keep'"
if ! "$converter" "$dir_path" >/dev/null 2>&1; then
    fail "Conversion failed"
fi
if ! sudo btrfs subvolume show "$dir_path" >/dev/null 2>&1; then
    fail "Converted path is not a subvolume"
fi
if ! sudo test -f "$dir_path/file.txt"; then
    fail "Missing file after conversion"
fi
if ! sudo test -f "$dir_path/.hidden/keep"; then
    fail "Missing dotfile after conversion"
fi
if ! sudo test "$(sudo cat "$dir_path/file.txt")" = "hello"; then
    fail "File content mismatch after conversion"
fi

sudo btrfs subvolume delete "$dir_path" >/dev/null

echo "All tests passed."
