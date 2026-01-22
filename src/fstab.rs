//! Fstab entry formatting and output.

/// Escape special characters for fstab output.
///
/// fstab(5) requires special characters to be octal-escaped because the line
/// is split on whitespace before parsing. Characters that need escaping:
/// - Space (\040), Tab (\011), Newline (\012), CR (\015) - field separators
/// - Backslash (\134) - escape character itself
/// - Hash (\043) - comment character at start of field
pub fn escape_fstab(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2); // Worst case: all chars escaped
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\134"),
            ' ' => result.push_str("\\040"),
            '\t' => result.push_str("\\011"),
            '\n' => result.push_str("\\012"),
            '\r' => result.push_str("\\015"), // Carriage return
            '#' => result.push_str("\\043"),
            _ => result.push(c),
        }
    }
    result
}

/// Convert an absolute mount target to a path relative to the root.
///
/// Returns "/" for the root mount, or the relative path for submounts.
/// Handles edge cases like empty strings gracefully.
pub fn make_fstab_target(target: &str, root_str: &str) -> String {
    // Handle empty input
    if target.is_empty() {
        return "/".to_string();
    }

    if target == root_str {
        "/".to_string()
    } else {
        let stripped = target.strip_prefix(root_str).unwrap_or(target);
        if stripped.is_empty() || stripped.starts_with('/') {
            if stripped.is_empty() {
                "/".to_string()
            } else {
                stripped.to_string()
            }
        } else {
            format!("/{}", stripped)
        }
    }
}

/// Determine the fsck pass number for a filesystem.
///
/// - Pass 1: Root filesystem (checked first)
/// - Pass 2: Other filesystems that support fsck
/// - Pass 0: Filesystems that don't need/support fsck
pub fn determine_pass_number(fstab_target: &str, fstype: &str) -> u8 {
    if fstab_target == "/" {
        1
    } else if needs_fsck(fstype) {
        2
    } else {
        0
    }
}

/// Check if a filesystem type supports/needs fsck at boot.
///
/// Note: vfat is excluded because EFI system partitions don't need fsck
/// (they're rarely written to after initial setup, and dosfsck has issues).
/// Note: btrfs is excluded because it doesn't use traditional fsck - it uses
/// `btrfs check` which should only be run manually, not at boot.
pub fn needs_fsck(fstype: &str) -> bool {
    matches!(fstype, "ext2" | "ext3" | "ext4" | "xfs" | "f2fs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::unescape_findmnt;

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
    fn test_determine_pass_number() {
        // Root always gets pass 1
        assert_eq!(determine_pass_number("/", "ext4"), 1);
        assert_eq!(determine_pass_number("/", "btrfs"), 1);

        // Filesystems that need fsck get pass 2
        assert_eq!(determine_pass_number("/boot", "ext4"), 2);
        assert_eq!(determine_pass_number("/home", "ext4"), 2);
        assert_eq!(determine_pass_number("/data", "xfs"), 2);

        // btrfs doesn't use fsck at boot (uses btrfs check manually)
        assert_eq!(determine_pass_number("/data", "btrfs"), 0);

        // vfat/EFI partitions don't need fsck (pass 0)
        assert_eq!(determine_pass_number("/boot/efi", "vfat"), 0);
        assert_eq!(determine_pass_number("/boot", "vfat"), 0);

        // Network filesystems don't need fsck
        assert_eq!(determine_pass_number("/nfs", "nfs"), 0);
        assert_eq!(determine_pass_number("/cifs", "cifs"), 0);
    }

    #[test]
    fn test_needs_fsck() {
        assert!(needs_fsck("ext2"));
        assert!(needs_fsck("ext3"));
        assert!(needs_fsck("ext4"));
        assert!(needs_fsck("xfs"));
        assert!(needs_fsck("f2fs"));

        // btrfs excluded (uses btrfs check manually, not fsck at boot)
        assert!(!needs_fsck("btrfs"));

        // vfat excluded (EFI partitions don't need fsck)
        assert!(!needs_fsck("vfat"));

        assert!(!needs_fsck("nfs"));
        assert!(!needs_fsck("cifs"));
        assert!(!needs_fsck("ntfs"));
        assert!(!needs_fsck("zfs"));
    }

    #[test]
    fn test_make_fstab_target_non_matching_prefix() {
        // When target doesn't start with root, strip_prefix returns None
        // and we fall back to the original (with leading slash added if needed)
        assert_eq!(make_fstab_target("/other/path", "/mnt"), "/other/path");
    }

    #[test]
    fn test_needs_fsck_case_sensitive() {
        assert!(needs_fsck("ext4"));
        assert!(!needs_fsck("EXT4"));
        assert!(!needs_fsck("Ext4"));
    }

    #[test]
    fn test_determine_pass_root_any_fstype() {
        // Root always gets pass 1 regardless of fstype
        assert_eq!(determine_pass_number("/", "ext4"), 1);
        assert_eq!(determine_pass_number("/", "btrfs"), 1);
        assert_eq!(determine_pass_number("/", "xfs"), 1);
        assert_eq!(determine_pass_number("/", "nfs"), 1); // Even network fs at root
        assert_eq!(determine_pass_number("/", "tmpfs"), 1); // Even tmpfs at root
    }

    #[test]
    fn test_escape_fstab_empty() {
        // Empty string should pass through
        assert_eq!(escape_fstab(""), "");
    }

    #[test]
    fn test_escape_fstab_unicode() {
        // Unicode characters should pass through unescaped
        assert_eq!(escape_fstab("/mnt/日本語"), "/mnt/日本語");
        assert_eq!(escape_fstab("/mnt/émoji🎉"), "/mnt/émoji🎉");
    }

    #[test]
    fn test_make_fstab_target_root_equals_target() {
        // When mount target equals root exactly
        assert_eq!(make_fstab_target("/mnt", "/mnt"), "/");
        assert_eq!(make_fstab_target("/", "/"), "/");
        assert_eq!(make_fstab_target("/a/b/c", "/a/b/c"), "/");
    }

    #[test]
    fn test_make_fstab_target_deeply_nested() {
        assert_eq!(
            make_fstab_target("/mnt/a/b/c/d/e/f", "/mnt"),
            "/a/b/c/d/e/f"
        );
    }

    #[test]
    fn test_escape_fstab() {
        // Spaces become \040
        assert_eq!(escape_fstab("/mnt/my disk"), "/mnt/my\\040disk");

        // Tabs become \011
        assert_eq!(escape_fstab("/mnt/tab\there"), "/mnt/tab\\011here");

        // Newlines become \012
        assert_eq!(escape_fstab("/mnt/new\nline"), "/mnt/new\\012line");

        // Backslashes become \134
        assert_eq!(escape_fstab("/mnt/back\\slash"), "/mnt/back\\134slash");

        // Hash becomes \043 (comment character)
        assert_eq!(escape_fstab("/mnt/#weird"), "/mnt/\\043weird");

        // Multiple escapes
        assert_eq!(
            escape_fstab("/mnt/my disk\\here"),
            "/mnt/my\\040disk\\134here"
        );

        // No escaping needed
        assert_eq!(escape_fstab("/mnt/normal"), "/mnt/normal");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        // Unescaping findmnt then escaping for fstab should handle spaces
        let findmnt_output = "/mnt/my\\x20disk";
        let unescaped = unescape_findmnt(findmnt_output);
        assert_eq!(unescaped, "/mnt/my disk");
        let fstab_escaped = escape_fstab(&unescaped);
        assert_eq!(fstab_escaped, "/mnt/my\\040disk");
    }

    #[test]
    fn test_escape_fstab_carriage_return() {
        // Carriage returns (\r) become \015 in fstab format
        // This is a rare but possible edge case (e.g., paths created from Windows)
        assert_eq!(escape_fstab("/mnt/cr\rhere"), "/mnt/cr\\015here");
        // Multiple carriage returns
        assert_eq!(escape_fstab("/mnt/a\rb\rc"), "/mnt/a\\015b\\015c");
        // CR + LF (Windows line ending in path)
        assert_eq!(escape_fstab("/mnt/crlf\r\nhere"), "/mnt/crlf\\015\\012here");
    }
}
