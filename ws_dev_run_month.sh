#!/bin/bash
set -euo pipefail

# ws_dev_run_month.sh
# Usage: ws_dev_run_month.sh YYYY-MM

LABEL="$1"

# Load Config
CONFIG_FILE="/home/chuck/.config/dev-backup/config.env"
if [ -f "$CONFIG_FILE" ]; then
    source "$CONFIG_FILE"
else
    # Defaults
    LS_HOST="localhost"
    LS_USER="chuck"
fi

TMP_MANIFEST="/tmp/snapshots_v2.tsv"

echo "Fetching manifest from LS ($LS_HOST)..."
if [ "$LS_HOST" == "localhost" ]; then
    cp "/srv/btrfs-backups/dev/manifests/snapshots_v2.tsv" "$TMP_MANIFEST" || touch "$TMP_MANIFEST"
else
    scp "$LS_USER@$LS_HOST:/srv/btrfs-backups/dev/manifests/snapshots_v2.tsv" "$TMP_MANIFEST" || touch "$TMP_MANIFEST"
fi

# Analyze Policy
# Default to Anchor if manifest empty
TYPE="anchor"
PARENT_LABEL=""

if [ -s "$TMP_MANIFEST" ]; then
    # Find last Anchor
    LAST_ANCHOR_INFO=$(grep -P "\tanchor\t" "$TMP_MANIFEST" | tail -n 1)
    
    if [ -n "$LAST_ANCHOR_INFO" ]; then
        ANCHOR_TS=$(echo "$LAST_ANCHOR_INFO" | awk '{print $1}')
        ANCHOR_BYTES=$(echo "$LAST_ANCHOR_INFO" | awk '{print $5}')
        # If ANCHOR_BYTES is empty/0 (first run?), treat as small.
        ANCHOR_BYTES=${ANCHOR_BYTES:-1}
        
        # Calculate time diff
        CURRENT_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
        # Convert to seconds (requires date that supports ISO 8601 parsing or simple hack)
        # Using date -d (GNU date)
        ANCHOR_SEC=$(date -d "$ANCHOR_TS" +%s)
        CURRENT_SEC=$(date -d "$CURRENT_TS" +%s)
        DIFF_MONTHS=$(( (CURRENT_SEC - ANCHOR_SEC) / 2592000 )) # Approx 30 days
        
        # Calculate Sum of Incrs since Anchor
        # We find lines AFTER the anchor
        # We can use awk to sum bytes where row number > anchor row number
        # Or simpler: grep -A 99999 matching anchor? No.
        
        # We'll use awk to find the anchor and sum subsequent lines
        # ts | label | type | parent | bytes ...
        SUM_INCR=$(awk -v anchor_ts="$ANCHOR_TS" -F'\t' '\\
            $1 == anchor_ts { found=1; next }
            found == 1 { sum += $5 }
            END { print sum }
        ' "$TMP_MANIFEST")
        SUM_INCR=${SUM_INCR:-0}
        
        echo "Last Anchor: $ANCHOR_TS (Size: $ANCHOR_BYTES)"
        echo "Sum Incr since: $SUM_INCR"
        echo "Months since: $DIFF_MONTHS"
        
        # Policy Check
        if [ "$DIFF_MONTHS" -ge 12 ]; then
            echo "Policy: >12 months since last anchor. New Anchor."
            TYPE="anchor"
        elif [ "$SUM_INCR" -ge "$ANCHOR_BYTES" ]; then
             echo "Policy: Incremental size ($SUM_INCR) >= Last Anchor size ($ANCHOR_BYTES). New Anchor."
             TYPE="anchor"
        else
             echo "Policy: Within limits. Incremental."
             TYPE="incremental"
             # Find Parent (Last snapshot)
             PARENT_LABEL=$(tail -n +2 "$TMP_MANIFEST" | tail -n 1 | awk '{print $2}')
             if [ -z "$PARENT_LABEL" ]; then
                 # Should not happen if anchor exists
                 TYPE="anchor"
             fi
        fi
    else
        echo "No existing anchor found. New Anchor."
        TYPE="anchor"
    fi
else
    echo "Manifest empty or missing. New Anchor."
    TYPE="anchor"
fi

# Execute
echo "Decision: $TYPE"

# Snapshot
/usr/local/bin/ws_dev_snapshot.sh "$LABEL"

# Export and Push
if [ "$TYPE" == "anchor" ]; then
    /usr/local/bin/ws_dev_export_and_push.sh "$LABEL"
else
    /usr/local/bin/ws_dev_export_and_push.sh "$LABEL" "$PARENT_LABEL"
fi

echo "Monthly run complete."
