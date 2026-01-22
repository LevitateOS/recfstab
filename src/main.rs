//! recfstab - Generate fstab from mounted filesystems
//!
//! Like genfstab for Arch Linux - reads mounts and outputs fstab entries.
//! Does ONE thing: generate fstab. User redirects output to file.
//!
//! Usage:
//!   recfstab /mnt >> /mnt/etc/fstab
//!
//! This is NOT an installer. This generates fstab. That's it.

use anyhow::{bail, Result};
use clap::Parser;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

#[derive(Parser)]
#[command(name = "recfstab")]
#[command(about = "Generate fstab from mounted filesystems (like genfstab)")]
struct Args {
    /// Root directory to scan (e.g., /mnt)
    root: String,

    /// Use LABEL instead of UUID
    #[arg(short = 'L', long)]
    label: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let root = Path::new(&args.root);
    if !root.exists() {
        bail!("Root directory {} does not exist", args.root);
    }
    if !root.is_dir() {
        bail!("{} is not a directory", args.root);
    }

    // Canonicalize the root path for matching
    let root_canonical = root.canonicalize()?;
    let root_str = root_canonical.to_string_lossy();

    // Get all mounts using findmnt
    let output = Command::new("findmnt")
        .args(["-rn", "-o", "TARGET,SOURCE,FSTYPE,OPTIONS"])
        .output()?;

    if !output.status.success() {
        bail!("findmnt failed");
    }

    let mounts_str = String::from_utf8_lossy(&output.stdout);
    let mut seen_targets: HashSet<String> = HashSet::new();

    // Process each mount
    for line in mounts_str.lines() {
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 4 {
            continue;
        }

        let target = parts[0];
        let source = parts[1];
        let fstype = parts[2];
        let options = parts[3];

        // Skip if not under our root
        if !target.starts_with(root_str.as_ref()) && target != root_str.as_ref() {
            continue;
        }

        // Skip pseudo-filesystems and special mounts
        if matches!(fstype, "proc" | "sysfs" | "devtmpfs" | "tmpfs" | "devpts" |
            "cgroup" | "cgroup2" | "efivarfs" | "securityfs" | "debugfs" | "tracefs" |
            "fusectl" | "configfs" | "binfmt_misc" | "autofs" | "mqueue" | "hugetlbfs" |
            "pstore" | "bpf" | "selinuxfs" | "binder" | "rpc_pipefs" | "fuse.portal" |
            "fuse.gvfsd-fuse" | "overlay") {
            continue;
        }

        // Skip if we've already seen this target (avoid duplicates)
        if seen_targets.contains(target) {
            continue;
        }
        seen_targets.insert(target.to_string());

        // Determine the fstab target path (relative to root)
        let fstab_target = if target == root_str.as_ref() {
            "/".to_string()
        } else {
            let stripped = target.strip_prefix(root_str.as_ref()).unwrap_or(target);
            if stripped.starts_with('/') {
                stripped.to_string()
            } else {
                format!("/{}", stripped)
            }
        };

        // Get UUID or LABEL for the device
        let identifier = get_device_identifier(source, args.label)?;

        // Determine pass number (1 for root, 2 for others, 0 for special fs)
        let pass = if fstab_target == "/" {
            1
        } else if needs_fsck(fstype) {
            2
        } else {
            0
        };

        // Filter options - remove some runtime-only options
        let filtered_options = filter_options(options);

        // Output fstab line
        println!(
            "# {source}");
        println!(
            "{}\t{}\t{}\t{}\t0\t{}",
            identifier, fstab_target, fstype, filtered_options, pass
        );
        println!();
    }

    Ok(())
}

fn get_device_identifier(source: &str, use_label: bool) -> Result<String> {
    // If it's already a UUID= or LABEL= reference, use it
    if source.starts_with("UUID=") || source.starts_with("LABEL=") {
        return Ok(source.to_string());
    }

    // Extract device path (handles btrfs subvolume notation like /dev/sda1[/subvol])
    let device = if let Some(bracket_pos) = source.find('[') {
        &source[..bracket_pos]
    } else {
        source
    };

    // For device paths, look up UUID or LABEL
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

    // For other sources (like bind mounts), use as-is
    Ok(source.to_string())
}

fn needs_fsck(fstype: &str) -> bool {
    matches!(fstype, "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "vfat" | "f2fs")
}

fn filter_options(options: &str) -> String {
    options
        .split(',')
        .filter(|opt| {
            // Remove runtime-only options
            !matches!(*opt, "seclabel" | "relatime" | "noatime" | "lazytime")
                && !opt.starts_with("subvolid=")
        })
        .collect::<Vec<_>>()
        .join(",")
}
