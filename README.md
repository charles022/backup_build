# TCLD Tools

## Reflink-enabled rsync

Use the setup script to build and install rsync with `--clone-dest` support:

```bash
./setup_rsync_reflink.sh
```

Details are in `rsync_reflink_build.md`.

## Directory to subvolume conversion

Convert a directory to a btrfs subvolume (requires reflink-enabled rsync):

```bash
./convert_to_subvolume.sh /path/to/dir
```

Run the test harness:

```bash
./test_convert_to_subvolume.sh
```

The test includes an "in-use directory" case (it will start a `tail -f`
and confirm the converter refuses to run while the directory is active).
