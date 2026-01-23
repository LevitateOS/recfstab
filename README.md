# recfstab

LevitateOS fstab generator. Like `genfstab` for Arch Linux - reads mounted filesystems and outputs fstab entries with UUIDs.

## Status

| Metric | Value |
|--------|-------|
| Stage | Beta |
| Target | x86_64 Linux |
| Last verified | 2026-01-23 |

### Works

- UUID, LABEL, PARTUUID, PARTLABEL identification modes
- Swap partition detection (skips zram)
- Pseudo-filesystem filtering

### Known Issues

- See parent repo issues

---

## Author

<!-- HUMAN WRITTEN - DO NOT MODIFY -->

[Waiting for human input]

<!-- END HUMAN WRITTEN -->

---

**You redirect the output yourself.** This tool generates fstab, nothing more.

## Usage

```bash
# After mounting your partitions
mount /dev/vda2 /mnt
mkdir -p /mnt/boot
mount /dev/vda1 /mnt/boot

# Generate fstab
recfstab /mnt >> /mnt/etc/fstab
```

## Options

```
USAGE:
    recfstab [OPTIONS] <ROOT>

ARGS:
    <ROOT>    Root directory to scan (e.g., /mnt)

OPTIONS:
    -L, --label       Use filesystem LABEL instead of UUID
    -p, --partuuid    Use partition UUID (PARTUUID)
    -t, --partlabel   Use partition LABEL (PARTLABEL)
    -h, --help        Print help
    -V, --version     Print version
```

## Examples

```bash
# Generate fstab with UUIDs (default)
recfstab /mnt >> /mnt/etc/fstab

# Generate fstab with LABELs
recfstab -L /mnt >> /mnt/etc/fstab

# Generate fstab with PARTUUIDs (useful for GPT disks)
recfstab -p /mnt >> /mnt/etc/fstab

# Preview output without writing
recfstab /mnt
```

## Sample Output

```
# /dev/vda2
UUID=a1b2c3d4-e5f6-7890-abcd-ef1234567890    /         ext4    defaults    0    1

# /dev/vda1
UUID=ABCD-1234                                /boot     vfat    defaults    0    2
```

## What recfstab does

- Reads mounted filesystems under the specified root
- Detects swap partitions from `/proc/swaps` (skips zram)
- Looks up UUIDs/LABELs/PARTUUIDs/PARTLABELs via blkid
- Outputs fstab-formatted entries
- Filters out pseudo-filesystems (proc, sysfs, tmpfs, etc.)

## What recfstab does NOT do

- Write to files directly (you redirect output)
- Modify existing fstab entries
- Mount or unmount anything
- Any other installation step

This is intentional. LevitateOS is for users who want control, like Arch.

## Error Codes

| Code | Exit | Description |
|------|------|-------------|
| E001 | 1 | Root directory does not exist |
| E002 | 2 | Path is not a directory |
| E003 | 3 | Failed to determine current directory |
| E004 | 4 | findmnt command not found (util-linux not installed) |
| E005 | 5 | findmnt command failed |
| E006 | 6 | No filesystems found under specified root |
| E007 | 7 | blkid command not found (util-linux not installed) |

## Requirements

- Root privileges required (for blkid UUID lookups)
- util-linux must be installed (provides findmnt and blkid)
- Target filesystems must be mounted

## Building

```bash
cargo build --release
```

## License

MIT
