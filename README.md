# TCLD Tools

## Reflink-enabled rsync

Use the setup script to build and install rsync with `--clone-dest` support:

```bash
./setup_rsync_reflink.sh
```

Details are in `rsync_reflink_build.md`.

## Directory to subvolume conversion

Convert a directory to a btrfs subvolume (uses `cp --reflink` for efficiency):

```bash
./convert_to_subvolume.sh /path/to/dir
```

## Snapshot Verification

To list all created snapshots:

```bash
sudo btrfs subvolume list -s /snapshot
```

To view space usage (referenced vs. exclusive data) for each snapshot:

```bash
sudo btrfs filesystem du -s -h /snapshot/*/*
```
