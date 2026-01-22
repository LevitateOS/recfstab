//! Filtering logic for mount entries and options.

/// Pseudo-filesystems and special mounts that should be excluded from fstab.
pub const PSEUDO_FILESYSTEMS: &[&str] = &[
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
pub const RUNTIME_OPTIONS: &[&str] = &["lazytime", "noatime", "relatime", "ro", "rw", "seclabel"];

/// Check if a filesystem type is a pseudo-filesystem that should be excluded.
pub fn is_pseudo_filesystem(fstype: &str) -> bool {
    PSEUDO_FILESYSTEMS.contains(&fstype)
}

/// Filter out runtime-only mount options that shouldn't be in fstab.
///
/// Handles edge cases like leading/trailing commas, empty options, and whitespace.
pub fn filter_options(options: &str) -> String {
    let filtered: Vec<&str> = options
        .split(',')
        .map(|opt| opt.trim()) // Handle whitespace around options
        .filter(|opt| {
            !opt.is_empty() && !RUNTIME_OPTIONS.contains(opt) && !opt.starts_with("subvolid=")
        })
        .collect();

    if filtered.is_empty() {
        "defaults".to_string()
    } else {
        filtered.join(",")
    }
}

/// Check if a filesystem type is empty or whitespace.
pub fn is_valid_fstype(fstype: &str) -> bool {
    !fstype.trim().is_empty()
}

/// Check if a mount target is under the given root path.
pub fn is_under_root(target: &str, root_str: &str) -> bool {
    if root_str == "/" {
        true // Everything is under "/"
    } else {
        target == root_str || target.starts_with(&format!("{}/", root_str))
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
    fn test_mount_filtering_by_root() {
        // These should match
        assert!(is_under_root("/mnt", "/mnt"));
        assert!(is_under_root("/mnt/boot", "/mnt"));
        assert!(is_under_root("/mnt/home/user", "/mnt"));

        // These should NOT match
        assert!(!is_under_root("/mnt2", "/mnt"));
        assert!(!is_under_root("/mntextra", "/mnt"));
        assert!(!is_under_root("/other", "/mnt"));

        // Root "/" matches everything
        assert!(is_under_root("/anything", "/"));
        assert!(is_under_root("/mnt/boot", "/"));
    }

    #[test]
    fn test_filter_options_empty() {
        assert_eq!(filter_options(""), "defaults");
    }

    #[test]
    fn test_filter_options_single() {
        assert_eq!(filter_options("rw"), "defaults");
        assert_eq!(filter_options("compress=zstd"), "compress=zstd");
    }

    #[test]
    fn test_filter_options_all_runtime() {
        // All options are runtime-only
        assert_eq!(
            filter_options("rw,relatime,lazytime,noatime,seclabel,ro"),
            "defaults"
        );
    }

    #[test]
    fn test_is_pseudo_filesystem_case_sensitive() {
        // Filesystem types are case-sensitive
        assert!(is_pseudo_filesystem("tmpfs"));
        assert!(!is_pseudo_filesystem("TMPFS"));
        assert!(!is_pseudo_filesystem("Tmpfs"));
    }

    #[test]
    fn test_pseudo_filesystems_comprehensive() {
        // Iterate over the actual constant to catch any additions
        for fs in PSEUDO_FILESYSTEMS {
            assert!(is_pseudo_filesystem(fs), "{} should be pseudo", fs);
        }
        // Verify the list is not empty (sanity check)
        assert!(
            PSEUDO_FILESYSTEMS.len() > 20,
            "Expected many pseudo filesystems"
        );
    }

    #[test]
    fn test_real_filesystems_not_pseudo() {
        let real = [
            "ext2", "ext3", "ext4", "xfs", "btrfs", "vfat", "ntfs", "f2fs", "zfs", "nfs", "nfs4",
            "cifs", "smb3", "exfat", "hfsplus",
        ];

        for fs in real {
            assert!(!is_pseudo_filesystem(fs), "{} should NOT be pseudo", fs);
        }
    }

    #[test]
    fn test_runtime_options_comprehensive() {
        // Iterate over the actual constant to catch any additions
        for opt in RUNTIME_OPTIONS {
            assert_eq!(
                filter_options(opt),
                "defaults",
                "{} should be filtered",
                opt
            );
        }
        // Verify the list is not empty (sanity check)
        assert!(
            RUNTIME_OPTIONS.len() >= 5,
            "Expected several runtime options"
        );
    }

    #[test]
    fn test_subvolid_filtered_subvol_kept() {
        // subvolid should be filtered, subvol should be kept
        assert_eq!(
            filter_options("subvolid=256,subvol=/@home"),
            "subvol=/@home"
        );
        assert_eq!(filter_options("rw,subvolid=5,subvol=/"), "subvol=/");
    }

    #[test]
    fn test_is_under_root_prefix_attack() {
        // Regression test: /mntextra should NOT match /mnt
        // This was a bug where starts_with("/mnt") matched "/mntextra"
        assert!(!is_under_root("/mntextra", "/mnt"));
        assert!(!is_under_root("/mnt2", "/mnt"));
        assert!(!is_under_root("/mnt-backup", "/mnt"));
        assert!(!is_under_root("/mnt_old", "/mnt"));
    }

    #[test]
    fn test_filter_options_leading_trailing_commas() {
        // Leading comma
        assert_eq!(filter_options(",rw,compress=zstd"), "compress=zstd");
        // Trailing comma
        assert_eq!(filter_options("rw,compress=zstd,"), "compress=zstd");
        // Both
        assert_eq!(filter_options(",rw,compress=zstd,"), "compress=zstd");
    }

    #[test]
    fn test_filter_options_multiple_commas() {
        // Multiple consecutive commas
        assert_eq!(filter_options("rw,,compress=zstd"), "compress=zstd");
        assert_eq!(
            filter_options("rw,,,noatime,,,compress=zstd"),
            "compress=zstd"
        );
    }

    #[test]
    fn test_filter_options_whitespace() {
        // Options with whitespace (shouldn't happen but handle gracefully)
        assert_eq!(filter_options(" rw , compress=zstd "), "compress=zstd");
    }

    #[test]
    fn test_is_valid_fstype() {
        assert!(is_valid_fstype("ext4"));
        assert!(is_valid_fstype("btrfs"));
        assert!(!is_valid_fstype(""));
        assert!(!is_valid_fstype("   "));
    }

    #[test]
    fn test_x_systemd_options_preserved() {
        // x-systemd.* options should pass through the filter (not in RUNTIME_OPTIONS)
        assert_eq!(
            filter_options("rw,x-systemd.automount"),
            "x-systemd.automount"
        );
        assert_eq!(
            filter_options("rw,x-systemd.device-timeout=30"),
            "x-systemd.device-timeout=30"
        );
        assert_eq!(
            filter_options("rw,x-systemd.idle-timeout=60,x-systemd.automount"),
            "x-systemd.idle-timeout=60,x-systemd.automount"
        );
        assert_eq!(
            filter_options("rw,relatime,x-systemd.mount-timeout=10s"),
            "x-systemd.mount-timeout=10s"
        );
    }

    #[test]
    fn test_ntfs3_not_pseudo() {
        // ntfs3 (kernel driver) is a real disk filesystem, not pseudo
        assert!(!is_pseudo_filesystem("ntfs3"));
        // fuse.ntfs-3g is also a real filesystem (FUSE-based NTFS)
        assert!(!is_pseudo_filesystem("fuse.ntfs-3g"));
    }
}
