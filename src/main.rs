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

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// Command-line arguments for recfstab.
#[derive(Parser, Debug)]
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
    "autofs",
    "binder",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fuse.gvfsd-fuse",
    "fuse.portal",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "overlay",
    "proc",
    "pstore",
    "ramfs",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "sysfs",
    "tmpfs",
    "tracefs",
];

/// Mount options that are runtime-only and should not appear in fstab.
const RUNTIME_OPTIONS: &[&str] = &["lazytime", "noatime", "relatime", "ro", "rw", "seclabel"];

fn main() -> Result<()> {
    let args = Args::parse();
    run(&args.root, args.label)
}

/// Main entry point for the fstab generator.
///
/// This function is separated from `main` to facilitate testing.
fn run(root_path: &str, use_label: bool) -> Result<()> {
    let root = Path::new(root_path);

    // Validate root directory
    if !root.exists() {
        bail!("Root directory '{}' does not exist", root_path);
    }
    if !root.is_dir() {
        bail!("'{}' is not a directory", root_path);
    }

    // Canonicalize the root path for accurate matching
    let root_canonical = root
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path '{}'", root_path))?;
    let root_str = root_canonical.to_string_lossy();

    // Get all mounts using findmnt
    let mounts = get_mounts()?;
    let mut seen_targets: HashSet<String> = HashSet::new();
    let mut found_any = false;

    for mount in mounts {
        // Skip mounts not under our root
        if !mount.target.starts_with(root_str.as_ref()) && mount.target != root_str.as_ref() {
            continue;
        }

        // Skip pseudo-filesystems
        if is_pseudo_filesystem(&mount.fstype) {
            continue;
        }

        // Skip duplicates
        if seen_targets.contains(&mount.target) {
            continue;
        }
        seen_targets.insert(mount.target.clone());

        // Convert absolute target path to path relative to root
        let fstab_target = make_fstab_target(&mount.target, &root_str);

        // Get UUID or LABEL for the device
        let identifier = get_device_identifier(&mount.source, use_label);

        // Determine fsck pass number
        let pass = determine_pass_number(&fstab_target, &mount.fstype);

        // Filter runtime-only mount options
        let filtered_options = filter_options(&mount.options);

        // Output fstab entry
        println!("# {}", mount.source);
        println!(
            "{}\t{}\t{}\t{}\t0\t{}",
            identifier, fstab_target, mount.fstype, filtered_options, pass
        );
        println!();

        found_any = true;
    }

    if !found_any {
        eprintln!(
            "Warning: No filesystems found under '{}'. \
             Make sure your target filesystems are mounted.",
            root_path
        );
    }

    Ok(())
}

/// Represents a single mount point from findmnt output.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MountInfo {
    target: String,
    source: String,
    fstype: String,
    options: String,
}

/// Get all current mounts from the system using findmnt.
fn get_mounts() -> Result<Vec<MountInfo>> {
    let output = Command::new("findmnt")
        .args(["-rn", "-o", "TARGET,SOURCE,FSTYPE,OPTIONS"])
        .output()
        .context("Failed to execute findmnt. Is util-linux installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "findmnt command failed: {}",
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.trim()
            }
        );
    }

    let mounts_str = String::from_utf8_lossy(&output.stdout);
    let mut mounts = Vec::new();

    for line in mounts_str.lines() {
        if let Some(mount) = parse_mount_line(line) {
            mounts.push(mount);
        }
    }

    Ok(mounts)
}

/// Parse a single line of findmnt output into a MountInfo struct.
fn parse_mount_line(line: &str) -> Option<MountInfo> {
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }

    Some(MountInfo {
        target: parts[0].to_string(),
        source: parts[1].to_string(),
        fstype: parts[2].to_string(),
        options: parts[3].to_string(),
    })
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

/// Extract the base device path from a source string.
///
/// Handles btrfs subvolume notation like `/dev/sda1[/subvol]`.
fn extract_device_path(source: &str) -> &str {
    if let Some(bracket_pos) = source.find('[') {
        &source[..bracket_pos]
    } else {
        source
    }
}

/// Get the device identifier (UUID or LABEL) for a source device.
///
/// Falls back to the device path if UUID/LABEL lookup fails.
fn get_device_identifier(source: &str, use_label: bool) -> String {
    // Already has an identifier
    if source.starts_with("UUID=") || source.starts_with("LABEL=") {
        return source.to_string();
    }

    let device = extract_device_path(source);

    // Look up UUID or LABEL for block devices
    if device.starts_with("/dev/") {
        if let Some(id) = lookup_device_id(device, use_label) {
            return id;
        }
        // Fall back to device path if no UUID/LABEL found
        return device.to_string();
    }

    // For other sources (bind mounts, network mounts), use as-is
    source.to_string()
}

/// Look up UUID or LABEL for a device using blkid.
fn lookup_device_id(device: &str, use_label: bool) -> Option<String> {
    let tag = if use_label { "LABEL" } else { "UUID" };
    let output = Command::new("blkid")
        .args(["-s", tag, "-o", "value", device])
        .output()
        .ok()?;

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(format!("{}={}", tag, value));
        }
    }

    None
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
        .filter(|opt| !RUNTIME_OPTIONS.contains(opt) && !opt.starts_with("subvolid="))
        .collect();

    if filtered.is_empty() {
        "defaults".to_string()
    } else {
        filtered.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pseudo_filesystem() {
        assert!(is_pseudo_filesystem("proc"));
        assert!(is_pseudo_filesystem("sysfs"));
        assert!(is_pseudo_filesystem("tmpfs"));
        assert!(is_pseudo_filesystem("devpts"));
        assert!(is_pseudo_filesystem("cgroup2"));

        assert!(!is_pseudo_filesystem("ext4"));
        assert!(!is_pseudo_filesystem("btrfs"));
        assert!(!is_pseudo_filesystem("vfat"));
        assert!(!is_pseudo_filesystem("xfs"));
    }

    #[test]
    fn test_make_fstab_target() {
        // Root mount
        assert_eq!(make_fstab_target("/mnt", "/mnt"), "/");

        // Submounts
        assert_eq!(make_fstab_target("/mnt/boot", "/mnt"), "/boot");
        assert_eq!(make_fstab_target("/mnt/home", "/mnt"), "/home");
        assert_eq!(make_fstab_target("/mnt/var/log", "/mnt"), "/var/log");

        // Edge case: trailing slash handling
        assert_eq!(make_fstab_target("/mnt/boot", "/mnt"), "/boot");
    }

    #[test]
    fn test_extract_device_path() {
        // Simple device path
        assert_eq!(extract_device_path("/dev/sda1"), "/dev/sda1");
        assert_eq!(extract_device_path("/dev/nvme0n1p1"), "/dev/nvme0n1p1");

        // Btrfs subvolume notation
        assert_eq!(extract_device_path("/dev/sda1[/root]"), "/dev/sda1");
        assert_eq!(extract_device_path("/dev/sda1[/home]"), "/dev/sda1");
        assert_eq!(
            extract_device_path("/dev/nvme0n1p3[/@snapshots]"),
            "/dev/nvme0n1p3"
        );

        // Non-device sources
        assert_eq!(extract_device_path("UUID=abc-123"), "UUID=abc-123");
        assert_eq!(extract_device_path("server:/share"), "server:/share");
    }

    #[test]
    fn test_determine_pass_number() {
        // Root always gets pass 1
        assert_eq!(determine_pass_number("/", "ext4"), 1);
        assert_eq!(determine_pass_number("/", "btrfs"), 1);

        // Filesystems that need fsck get pass 2
        assert_eq!(determine_pass_number("/boot", "ext4"), 2);
        assert_eq!(determine_pass_number("/home", "ext4"), 2);
        assert_eq!(determine_pass_number("/boot", "vfat"), 2);
        assert_eq!(determine_pass_number("/data", "xfs"), 2);
        assert_eq!(determine_pass_number("/data", "btrfs"), 2);

        // Filesystems that don't need fsck get pass 0
        assert_eq!(determine_pass_number("/boot/efi", "vfat"), 2);
        assert_eq!(determine_pass_number("/nfs", "nfs"), 0);
        assert_eq!(determine_pass_number("/cifs", "cifs"), 0);
    }

    #[test]
    fn test_needs_fsck() {
        assert!(needs_fsck("ext2"));
        assert!(needs_fsck("ext3"));
        assert!(needs_fsck("ext4"));
        assert!(needs_fsck("xfs"));
        assert!(needs_fsck("btrfs"));
        assert!(needs_fsck("vfat"));
        assert!(needs_fsck("f2fs"));

        assert!(!needs_fsck("nfs"));
        assert!(!needs_fsck("cifs"));
        assert!(!needs_fsck("ntfs"));
        assert!(!needs_fsck("zfs"));
    }

    #[test]
    fn test_filter_options() {
        // Remove runtime options
        assert_eq!(filter_options("rw,relatime,seclabel"), "defaults");
        assert_eq!(filter_options("rw,relatime,compress=zstd"), "compress=zstd");

        // Preserve important options
        assert_eq!(
            filter_options("rw,compress=zstd:1,ssd,space_cache=v2"),
            "compress=zstd:1,ssd,space_cache=v2"
        );

        // Remove subvolid but keep subvol
        assert_eq!(
            filter_options("rw,subvolid=256,subvol=/root"),
            "subvol=/root"
        );

        // Empty after filtering returns defaults
        assert_eq!(filter_options("rw"), "defaults");
        assert_eq!(filter_options("rw,relatime"), "defaults");

        // Complex vfat options
        assert_eq!(
            filter_options("rw,fmask=0077,dmask=0077,codepage=437"),
            "fmask=0077,dmask=0077,codepage=437"
        );
    }

    #[test]
    fn test_parse_mount_line() {
        let line = "/mnt /dev/sda1 ext4 rw,relatime";
        let mount = parse_mount_line(line).unwrap();
        assert_eq!(mount.target, "/mnt");
        assert_eq!(mount.source, "/dev/sda1");
        assert_eq!(mount.fstype, "ext4");
        assert_eq!(mount.options, "rw,relatime");

        // Line with spaces in options (splitn handles this)
        let line2 = "/mnt/boot /dev/sda2 vfat rw,fmask=0077,dmask=0077";
        let mount2 = parse_mount_line(line2).unwrap();
        assert_eq!(mount2.target, "/mnt/boot");
        assert_eq!(mount2.fstype, "vfat");

        // Invalid line (too few parts)
        assert!(parse_mount_line("/mnt /dev/sda1").is_none());
        assert!(parse_mount_line("").is_none());
    }

    #[test]
    fn test_get_device_identifier_existing_uuid() {
        // Already has UUID
        assert_eq!(
            get_device_identifier("UUID=abc-123-def", false),
            "UUID=abc-123-def"
        );
        assert_eq!(
            get_device_identifier("UUID=abc-123-def", true),
            "UUID=abc-123-def"
        );

        // Already has LABEL
        assert_eq!(get_device_identifier("LABEL=myroot", false), "LABEL=myroot");
        assert_eq!(get_device_identifier("LABEL=myroot", true), "LABEL=myroot");
    }

    #[test]
    fn test_get_device_identifier_non_device() {
        // Network mounts
        assert_eq!(
            get_device_identifier("server:/export", false),
            "server:/export"
        );
        assert_eq!(
            get_device_identifier("//server/share", false),
            "//server/share"
        );
    }
}
