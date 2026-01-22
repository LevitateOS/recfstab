# CLAUDE.md - Recfstab

## STOP. READ. THEN ACT.

Before modifying this crate, read `src/main.rs` to understand the fstab generation logic.

---

## What is recfstab?

LevitateOS fstab generator. **Like genfstab, NOT like an installer.**

Reads mounted filesystems under a root directory and outputs fstab entries. That's it.
User redirects output to a file. User does EVERYTHING else manually.

## Development

```bash
cargo build --release    # LTO + strip enabled
cargo clippy
```

## Key Rules

1. **recfstab = genfstab** - Just generate fstab, nothing else
2. **Keep it simple** - ~100 lines, one job
3. **No automation** - User redirects output manually

## What recfstab does

```bash
recfstab /mnt >> /mnt/etc/fstab    # Generate fstab with UUIDs
recfstab -L /mnt >> /mnt/etc/fstab # Generate fstab with LABELs
```

## What recfstab does NOT do

- Write to files directly (user redirects)
- Modify existing fstab (user manages)
- Install anything (user does that)
- Any other installation step

## Testing

Test with mounted filesystems, verify output format matches fstab(5).
