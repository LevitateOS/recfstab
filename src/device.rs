//! Device identifier lookup (UUID/LABEL/PARTUUID/PARTLABEL).

use std::process::Command;

/// Device identifier type for fstab entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdType {
    /// Use filesystem UUID (default)
    #[default]
    Uuid,
    /// Use filesystem LABEL
    Label,
    /// Use partition UUID (GPT PARTUUID)
    Partuuid,
    /// Use partition LABEL (GPT PARTLABEL)
    Partlabel,
}

impl IdType {
    /// Get the blkid tag name for this identifier type.
    pub fn blkid_tag(&self) -> &'static str {
        match self {
            IdType::Uuid => "UUID",
            IdType::Label => "LABEL",
            IdType::Partuuid => "PARTUUID",
            IdType::Partlabel => "PARTLABEL",
        }
    }

    /// Get the fstab prefix for this identifier type.
    pub fn fstab_prefix(&self) -> &'static str {
        match self {
            IdType::Uuid => "UUID",
            IdType::Label => "LABEL",
            IdType::Partuuid => "PARTUUID",
            IdType::Partlabel => "PARTLABEL",
        }
    }
}

/// Extract the base device path from a source string.
///
/// Handles btrfs subvolume notation like `/dev/sda1[/subvol]`.
/// Returns empty string if source is empty or starts with '['.
pub fn extract_device_path(source: &str) -> &str {
    if source.is_empty() {
        return "";
    }
    if let Some(bracket_pos) = source.find('[') {
        // Handle edge case where source starts with '['
        if bracket_pos == 0 {
            return "";
        }
        &source[..bracket_pos]
    } else {
        source
    }
}

/// Get the device identifier (UUID/LABEL/PARTUUID/PARTLABEL) for a source device.
///
/// Falls back to the device path if identifier lookup fails.
/// Preserves existing identifiers (UUID=, LABEL=, PARTUUID=, PARTLABEL=).
///
/// # Arguments
/// * `source` - The device source string (e.g., "/dev/sda1", "/dev/sda1[/subvol]")
/// * `id_type` - The identifier type to use ("UUID", "LABEL", "PARTUUID", "PARTLABEL")
pub fn get_device_identifier(source: &str, id_type: &str) -> String {
    // Handle empty source gracefully
    if source.is_empty() {
        return "none".to_string();
    }

    // Already has an identifier - preserve it
    if source.starts_with("UUID=")
        || source.starts_with("LABEL=")
        || source.starts_with("PARTUUID=")
        || source.starts_with("PARTLABEL=")
    {
        return source.to_string();
    }

    let device = extract_device_path(source);

    // Handle empty device after extraction
    if device.is_empty() {
        return source.to_string();
    }

    // Look up identifier for block devices
    if device.starts_with("/dev/") {
        if let Some(id) = lookup_device_id(device, id_type) {
            return id;
        }
        // Fall back to device path if no identifier found
        return device.to_string();
    }

    // For other sources (bind mounts, network mounts), use as-is
    source.to_string()
}

/// Look up an identifier for a device using blkid.
///
/// # Arguments
/// * `device` - The device path (e.g., "/dev/sda1")
/// * `tag` - The blkid tag to look up ("UUID", "LABEL", "PARTUUID", "PARTLABEL")
///
/// Returns None silently on failure - Unix philosophy: avoid unnecessary output.
/// The caller handles fallback to device path.
pub fn lookup_device_id(device: &str, tag: &str) -> Option<String> {
    let output = Command::new("blkid")
        .args(["-s", tag, "-o", "value", device])
        .output()
        .ok()?; // Silent failure - falls back to device path

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(format!("{}={}", tag, value));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_get_device_identifier_existing_uuid() {
        // Already has UUID - preserved regardless of id_type
        assert_eq!(
            get_device_identifier("UUID=abc-123-def", "UUID"),
            "UUID=abc-123-def"
        );
        assert_eq!(
            get_device_identifier("UUID=abc-123-def", "LABEL"),
            "UUID=abc-123-def"
        );

        // Already has LABEL - preserved
        assert_eq!(
            get_device_identifier("LABEL=myroot", "UUID"),
            "LABEL=myroot"
        );

        // Already has PARTUUID - preserved
        assert_eq!(
            get_device_identifier("PARTUUID=abc-123-def", "UUID"),
            "PARTUUID=abc-123-def"
        );

        // Already has PARTLABEL - preserved
        assert_eq!(
            get_device_identifier("PARTLABEL=myroot", "PARTUUID"),
            "PARTLABEL=myroot"
        );
    }

    #[test]
    fn test_get_device_identifier_non_device() {
        // Network mounts - returned as-is
        assert_eq!(
            get_device_identifier("server:/export", "UUID"),
            "server:/export"
        );
        assert_eq!(
            get_device_identifier("//server/share", "UUID"),
            "//server/share"
        );
    }

    #[test]
    fn test_extract_device_path_empty_subvol() {
        // Empty subvolume bracket
        assert_eq!(extract_device_path("/dev/sda1[]"), "/dev/sda1");
    }

    #[test]
    fn test_extract_device_path_nested_brackets() {
        // Nested brackets (unusual but handle gracefully)
        assert_eq!(extract_device_path("/dev/sda1[/[nested]]"), "/dev/sda1");
    }

    #[test]
    fn test_get_device_identifier_uuid_formats() {
        // Various UUID formats should pass through
        assert_eq!(
            get_device_identifier("UUID=550e8400-e29b-41d4-a716-446655440000", "UUID"),
            "UUID=550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(
            get_device_identifier("UUID=ABCD-1234", "UUID"), // FAT UUID format
            "UUID=ABCD-1234"
        );
    }

    #[test]
    fn test_get_device_identifier_label_formats() {
        // Various LABEL formats - preserved regardless of id_type
        assert_eq!(
            get_device_identifier("LABEL=my-root", "UUID"),
            "LABEL=my-root"
        );
        assert_eq!(get_device_identifier("LABEL=EFI", "UUID"), "LABEL=EFI");
        assert_eq!(
            get_device_identifier("LABEL=boot partition", "UUID"), // Space in label
            "LABEL=boot partition"
        );
    }

    #[test]
    fn test_get_device_identifier_nonexistent_device_fallback() {
        // When blkid fails (device doesn't exist), should fall back to device path
        let result = get_device_identifier("/dev/nonexistent_device_xyz123", "UUID");
        assert_eq!(result, "/dev/nonexistent_device_xyz123");
    }

    #[test]
    fn test_get_device_identifier_btrfs_subvol_fallback() {
        // Btrfs device with subvol that doesn't exist should extract device and fall back
        let result = get_device_identifier("/dev/nonexistent_xyz[/subvol]", "UUID");
        assert_eq!(result, "/dev/nonexistent_xyz");
    }

    #[test]
    fn test_id_type_blkid_tag() {
        assert_eq!(IdType::Uuid.blkid_tag(), "UUID");
        assert_eq!(IdType::Label.blkid_tag(), "LABEL");
        assert_eq!(IdType::Partuuid.blkid_tag(), "PARTUUID");
        assert_eq!(IdType::Partlabel.blkid_tag(), "PARTLABEL");
    }

    #[test]
    fn test_id_type_fstab_prefix() {
        assert_eq!(IdType::Uuid.fstab_prefix(), "UUID");
        assert_eq!(IdType::Label.fstab_prefix(), "LABEL");
        assert_eq!(IdType::Partuuid.fstab_prefix(), "PARTUUID");
        assert_eq!(IdType::Partlabel.fstab_prefix(), "PARTLABEL");
    }
}
