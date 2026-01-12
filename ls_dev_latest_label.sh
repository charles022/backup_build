#!/bin/bash
set -euo pipefail

# ls_dev_latest_label.sh
# Usage: ls_dev_latest_label.sh

MANIFEST="/srv/btrfs-backups/dev/manifests/snapshots_v2.tsv"
if [ ! -f "$MANIFEST" ]; then
    echo "none"
    exit 0
fi

# Sort by TS and take last
LAST_LABEL=$(tail -n +2 "$MANIFEST" | sort -k1 | tail -n 1 | awk '{print $2}')

if [ -z "$LAST_LABEL" ]; then
    echo "none"
else
    echo "$LAST_LABEL"
fi
