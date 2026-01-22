//! # recfstab
//!
//! Generate fstab entries from mounted filesystems.
//!
//! `recfstab` is a simple, focused tool that reads the currently mounted filesystems
//! under a specified root directory and outputs properly formatted fstab entries.
//! It's designed to work like Arch Linux's `genfstab` utility.
//!
//! ## Features
//!
//! - Generates fstab entries with UUID identifiers (default) or LABELs
//! - Automatically filters out pseudo-filesystems (proc, sysfs, tmpfs, etc.)
//! - Handles btrfs subvolumes correctly
//! - Outputs standard fstab format compatible with all Linux distributions
//!
//! ## Usage
//!
//! ```bash
//! # Generate fstab for filesystems mounted under /mnt
//! recfstab /mnt >> /mnt/etc/fstab
//!
//! # Use LABELs instead of UUIDs
//! recfstab -L /mnt >> /mnt/etc/fstab
//! ```
//!
//! ## Requirements
//!
//! - Linux system with `findmnt` and `blkid` utilities
//! - Root privileges (for blkid to read device UUIDs)

use anyhow::{bail, Result};
use clap::Parser;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// Command-line arguments for recfstab.
#[derive(Parser)]
#[command(name = "recfstab")]
#[command(version)]
#[command(about = "Generate fstab from mounted filesystems (like genfstab)")]
#[command(
    long_about = "Reads mounted filesystems under a root directory and outputs \
    fstab entries with UUIDs (or LABELs). Designed for system installation workflows \
    where you need to generate /etc/fstab for a newly installed system."
)]
struct Args {
    /// Root directory to scan for mounted filesystems (e.g., /mnt)
    root: String,

    /// Use filesystem LABEL instead of UUID for device identification
    #[arg(short = 'L', long)]
    label: bool,
}

/// Pseudo-filesystems and special mounts that should be excluded from fstab.
const PSEUDO_FILESYSTEMS: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "tmpfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "efivarfs",
    "securityfs",
    "debugfs",
    "tracefs",
    "fusectl",
    "configfs",
    "binfmt_misc",
    "autofs",
    "mqueue",
    "hugetlbfs",
    "pstore",
    "bpf",
    "selinuxfs",
    "binder",
    "rpc_pipefs",
    "fuse.portal",
    "fuse.gvfsd-fuse",
    "overlay",
    "ramfs",
    "nsfs",
];

fn main() -> Result<()> {
    let args = Args::parse();

    let root = Path::new(&args.root);
    if !root.exists() {
        bail!("Root directory '{}' does not exist", args.root);
    }
    if !root.is_dir() {
        bail!("'{}' is not a directory", args.root);
    }

    // Canonicalize the root path for accurate matching
    let root_canonical = root.canonicalize()?;
    let root_str = root_canonical.to_string_lossy();

    // Get all mounts using findmnt
    let output = Command::new("findmnt")
        .args(["-rn", "-o", "TARGET,SOURCE,FSTYPE,OPTIONS"])
        .output()?;

    if !output.status.success() {
        bail!(
            "findmnt command failed. Ensure util-linux is installed and you have \
             appropriate permissions."
        );
    }

    let mounts_str = String::from_utf8_lossy(&output.stdout);
    let mut seen_targets: HashSet<String> = HashSet::new();

    for line in mounts_str.lines() {
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 4 {
            continue;
        }

        let target = parts[0];
        let source = parts[1];
        let fstype = parts[2];
        let options = parts[3];

        // Skip mounts not under our root
        if !target.starts_with(root_str.as_ref()) && target != root_str.as_ref() {
            continue;
        }

        // Skip pseudo-filesystems
        if is_pseudo_filesystem(fstype) {
            continue;
        }

        // Skip duplicates
        if seen_targets.contains(target) {
            continue;
        }
        seen_targets.insert(target.to_string());

        // Convert absolute target path to path relative to root
        let fstab_target = make_fstab_target(target, &root_str);

        // Get UUID or LABEL for the device
        let identifier = get_device_identifier(source, args.label)?;

        // Determine fsck pass number
        let pass = determine_pass_number(&fstab_target, fstype);

        // Filter runtime-only mount options
        let filtered_options = filter_options(options);

        // Output fstab entry
        println!("# {}", source);
        println!(
            "{}\t{}\t{}\t{}\t0\t{}",
            identifier, fstab_target, fstype, filtered_options, pass
        );
        println!();
    }

    Ok(())
}

/// Check if a filesystem type is a pseudo-filesystem that should be excluded.
fn is_pseudo_filesystem(fstype: &str) -> bool {
    PSEUDO_FILESYSTEMS.contains(&fstype)
}

/// Convert an absolute mount target to a path relative to the root.
fn make_fstab_target(target: &str, root_str: &str) -> String {
    if target == root_str {
        "/".to_string()
    } else {
        let stripped = target.strip_prefix(root_str).unwrap_or(target);
        if stripped.starts_with('/') {
            stripped.to_string()
        } else {
            format!("/{}", stripped)
        }
    }
}

/// Get the device identifier (UUID or LABEL) for a source device.
fn get_device_identifier(source: &str, use_label: bool) -> Result<String> {
    // Already has an identifier
    if source.starts_with("UUID=") || source.starts_with("LABEL=") {
        return Ok(source.to_string());
    }

    // Extract device path (handles btrfs subvolume notation like /dev/sda1[/subvol])
    let device = if let Some(bracket_pos) = source.find('[') {
        &source[..bracket_pos]
    } else {
        source
    };

    // Look up UUID or LABEL for block devices
    if device.starts_with("/dev/") {
        let tag = if use_label { "LABEL" } else { "UUID" };
        let output = Command::new("blkid")
            .args(["-s", tag, "-o", "value", device])
            .output()?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return Ok(format!("{}={}", tag, value));
            }
        }

        // Fall back to device path if no UUID/LABEL found
        return Ok(device.to_string());
    }

    // For other sources (bind mounts, network mounts), use as-is
    Ok(source.to_string())
}

/// Determine the fsck pass number for a filesystem.
///
/// - Pass 1: Root filesystem (checked first)
/// - Pass 2: Other filesystems that support fsck
/// - Pass 0: Filesystems that don't need/support fsck
fn determine_pass_number(fstab_target: &str, fstype: &str) -> u8 {
    if fstab_target == "/" {
        1
    } else if needs_fsck(fstype) {
        2
    } else {
        0
    }
}

/// Check if a filesystem type supports/needs fsck.
fn needs_fsck(fstype: &str) -> bool {
    matches!(
        fstype,
        "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "vfat" | "f2fs"
    )
}

/// Filter out runtime-only mount options that shouldn't be in fstab.
fn filter_options(options: &str) -> String {
    let filtered: Vec<&str> = options
        .split(',')
        .filter(|opt| {
            !matches!(
                *opt,
                "seclabel" | "relatime" | "noatime" | "lazytime" | "rw" | "ro"
            ) && !opt.starts_with("subvolid=")
        })
        .collect();

    if filtered.is_empty() {
        "defaults".to_string()
    } else {
        filtered.join(",")
    }
}
