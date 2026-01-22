//! Mount point parsing from findmnt output.

use crate::error::{RecfstabError, Result};
use std::process::Command;

/// Represents a single mount point from findmnt output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    pub target: String,
    pub source: String,
    pub fstype: String,
    pub options: String,
}

/// Get all current mounts from the system using findmnt.
pub fn get_mounts() -> Result<Vec<MountInfo>> {
    let output = Command::new("findmnt")
        .args(["-rn", "-o", "TARGET,SOURCE,FSTYPE,OPTIONS"])
        .output()
        .map_err(RecfstabError::findmnt_not_found)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RecfstabError::findmnt_failed(&stderr));
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
///
/// Returns None if the line is malformed or has empty required fields.
pub fn parse_mount_line(line: &str) -> Option<MountInfo> {
    // Skip empty or whitespace-only lines
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return None;
    }

    let target = unescape_findmnt(parts[0]);
    let source = unescape_findmnt(parts[1]);
    let fstype = parts[2].to_string();
    let options = parts[3].to_string();

    // Validate required fields are not empty after unescaping
    if target.is_empty() || fstype.is_empty() {
        return None;
    }

    Some(MountInfo {
        target,
        source,
        fstype,
        options,
    })
}

/// Unescape special characters in findmnt -r output.
///
/// findmnt -r escapes spaces as \x20, tabs as \x09, newlines as \x0a, backslashes as \x5c.
/// Order matters: backslash must be unescaped LAST to avoid double-unescaping.
pub fn unescape_findmnt(s: &str) -> String {
    // Process backslash LAST to avoid turning \\x5cx20 into \ x20 then into \<space>
    s.replace("\\x20", " ")
        .replace("\\x09", "\t")
        .replace("\\x0a", "\n")
        .replace("\\x0d", "\r") // Carriage return (rare but possible)
        .replace("\\x5c", "\\") // Backslash MUST be last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unescape_findmnt() {
        // Space escaping
        assert_eq!(unescape_findmnt("/mnt/my\\x20disk"), "/mnt/my disk");
        assert_eq!(
            unescape_findmnt("/mnt/path\\x20with\\x20spaces"),
            "/mnt/path with spaces"
        );

        // Tab escaping
        assert_eq!(unescape_findmnt("/mnt/tab\\x09here"), "/mnt/tab\there");

        // Backslash escaping
        assert_eq!(unescape_findmnt("/mnt/back\\x5cslash"), "/mnt/back\\slash");

        // No escaping needed
        assert_eq!(unescape_findmnt("/mnt/normal"), "/mnt/normal");
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

        // Escaped spaces in mount path (findmnt -r output)
        let line3 = "/mnt/my\\x20disk /dev/sda3 ext4 rw,relatime";
        let mount3 = parse_mount_line(line3).unwrap();
        assert_eq!(mount3.target, "/mnt/my disk");
        assert_eq!(mount3.source, "/dev/sda3");

        // Invalid line (too few parts)
        assert!(parse_mount_line("/mnt /dev/sda1").is_none());
        assert!(parse_mount_line("").is_none());
    }

    #[test]
    fn test_unescape_findmnt_no_escapes() {
        // No escapes - should pass through unchanged
        assert_eq!(unescape_findmnt("/mnt/normal/path"), "/mnt/normal/path");
        assert_eq!(unescape_findmnt(""), "");
    }

    #[test]
    fn test_unescape_findmnt_multiple_escapes() {
        // Multiple escapes in sequence
        assert_eq!(unescape_findmnt("/mnt/a\\x20b\\x20c\\x20d"), "/mnt/a b c d");
    }

    #[test]
    fn test_unescape_findmnt_partial_escape() {
        // Partial escape sequence (malformed) - should pass through
        assert_eq!(unescape_findmnt("/mnt/\\x2"), "/mnt/\\x2");
        assert_eq!(unescape_findmnt("/mnt/\\x"), "/mnt/\\x");
    }

    #[test]
    fn test_findmnt_parse_error_handling() {
        // Empty line should return None
        assert!(parse_mount_line("").is_none());

        // Line with only 1 field
        assert!(parse_mount_line("/mnt").is_none());

        // Line with only 2 fields
        assert!(parse_mount_line("/mnt /dev/sda1").is_none());

        // Line with only 3 fields
        assert!(parse_mount_line("/mnt /dev/sda1 ext4").is_none());

        // Line with 4 fields should succeed
        assert!(parse_mount_line("/mnt /dev/sda1 ext4 rw").is_some());
    }

    #[test]
    fn test_parse_mount_line_options_with_spaces() {
        // Options field can contain anything after the 4th space (splitn(4))
        let line = "/mnt /dev/sda1 ext4 rw,user_xattr,some option with spaces";
        let mount = parse_mount_line(line).unwrap();
        assert_eq!(mount.options, "rw,user_xattr,some option with spaces");
    }

    #[test]
    fn test_unescape_findmnt_newline() {
        // Newline escaping
        assert_eq!(unescape_findmnt("/mnt/new\\x0aline"), "/mnt/new\nline");
    }
}
