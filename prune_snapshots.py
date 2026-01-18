#!/usr/bin/env python3
import os
import sys
import re
import datetime
import subprocess
from collections import defaultdict

# Configuration
SNAPSHOT_ROOT = "/snapshot"
RETENTION_DAYS = 30

def get_snapshot_date(filename):
    """
    Parses the timestamp from the snapshot filename.
    Expected format ends in: -YYYYMMDD_HHMM
    """
    # Regex to match the date pattern at the end of the string
    match = re.search(r"-(\d{8}_\d{4})$", filename)
    if match:
        date_str = match.group(1)
        try:
            return datetime.datetime.strptime(date_str, "%Y%m%d_%H%M")
        except ValueError:
            return None
    return None

def prune_snapshots():
    if not os.path.exists(SNAPSHOT_ROOT):
        print(f"Snapshot root {SNAPSHOT_ROOT} does not exist. Nothing to prune.")
        return

    # Check if we have sudo access for btrfs commands early
    try:
        subprocess.run(["sudo", "-n", "true"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        print("Error: This script requires sudo privileges to delete Btrfs subvolumes.")
        print("Please run with sudo or ensure the user has NOPASSWD sudo rights.")
        # We continue, but actual deletion might fail interactively if password is needed and not provided.

    now = datetime.datetime.now()
    cutoff_date = now - datetime.timedelta(days=RETENTION_DAYS)
    
    print(f"--- Pruning Snapshots ---")
    print(f"Current Date: {now.strftime('%Y-%m-%d %H:%M')}")
    print(f"30-Day Cutoff: {cutoff_date.strftime('%Y-%m-%d %H:%M')}")
    print(f"Policy: Keep last 30 days, then 1 per month (earliest).")
    print(f"-------------------------")

    # Iterate over each subvolume directory in /snapshot/
    # e.g. /snapshot/home, /snapshot/home-chuck-code
    for entry in os.scandir(SNAPSHOT_ROOT):
        if not entry.is_dir():
            continue
            
        subvol_name = entry.name
        subvol_dir = entry.path
        
        print(f"\nProcessing group: {subvol_name}")
        
        # Collect all snapshots in this group
        snapshots = []
        for snap in os.scandir(subvol_dir):
            if not snap.is_dir(): # Snapshots are directories (subvolumes)
                continue
                
            snap_date = get_snapshot_date(snap.name)
            if snap_date:
                snapshots.append({
                    'path': snap.path,
                    'name': snap.name,
                    'date': snap_date
                })
            else:
                print(f"  [WARN] Skipping unrecognized format: {snap.name}")

        # Sort by date ascending (oldest first)
        snapshots.sort(key=lambda x: x['date'])

        # Apply retention logic
        kept_months = set()
        to_delete = []
        kept_count = 0

        for snap in snapshots:
            snap_date = snap['date']
            
            # Logic 1: Keep last 30 days
            if snap_date > cutoff_date:
                # Keep
                kept_count += 1
                continue
            
            # Logic 2: Older than 30 days, keep 1 per month
            # Since list is sorted by date ascending, the first one we see 
            # for a given (year, month) is the earliest available.
            ym = (snap_date.year, snap_date.month)
            if ym not in kept_months:
                kept_months.add(ym)
                kept_count += 1
                # This is the "monthly keeper"
            else:
                # We already have a snapshot for this month, delete this one
                to_delete.append(snap)

        # Execute Deletion
        if not to_delete:
            print(f"  No snapshots to prune. (Kept: {kept_count})")
        else:
            print(f"  Deleting {len(to_delete)} snapshots (Kept: {kept_count})...")
            for item in to_delete:
                print(f"    Deleting: {item['name']}")
                try:
                    # Using sudo to delete
                    cmd = ["sudo", "btrfs", "subvolume", "delete", item['path']]
                    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)
                except subprocess.CalledProcessError as e:
                    print(f"    [ERROR] Failed to delete {item['name']}: {e}")

if __name__ == "__main__":
    prune_snapshots()
