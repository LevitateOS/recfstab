//! Error types with numbered codes for recfstab.
//!
//! Error codes make it easy to:
//! - Document specific failure modes
//! - Search for solutions
//! - Script around specific errors
//!
//! ## Error Code Reference
//!
//! | Code | Description |
//! |------|-------------|
//! | E001 | Root directory does not exist |
//! | E002 | Path is not a directory |
//! | E003 | Failed to determine current directory |
//! | E004 | findmnt command not found (util-linux not installed) |
//! | E005 | findmnt command failed |
//! | E006 | No filesystems found under specified root |

use std::fmt;

/// Error codes for recfstab failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// E001: Root directory does not exist
    RootNotFound,
    /// E002: Path is not a directory
    NotADirectory,
    /// E003: Failed to determine current directory
    CurrentDirFailed,
    /// E004: findmnt command not found
    FindmntNotFound,
    /// E005: findmnt command failed
    FindmntFailed,
    /// E006: No filesystems found under root
    NoFilesystems,
}

impl ErrorCode {
    /// Get the numeric code as a string (e.g., "E001").
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::RootNotFound => "E001",
            ErrorCode::NotADirectory => "E002",
            ErrorCode::CurrentDirFailed => "E003",
            ErrorCode::FindmntNotFound => "E004",
            ErrorCode::FindmntFailed => "E005",
            ErrorCode::NoFilesystems => "E006",
        }
    }

    /// Get a short description of the error.
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCode::RootNotFound => "root directory does not exist",
            ErrorCode::NotADirectory => "path is not a directory",
            ErrorCode::CurrentDirFailed => "failed to determine current directory",
            ErrorCode::FindmntNotFound => "findmnt command not found",
            ErrorCode::FindmntFailed => "findmnt command failed",
            ErrorCode::NoFilesystems => "no filesystems found",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// A recfstab error with code and context.
#[derive(Debug)]
pub struct RecfstabError {
    pub code: ErrorCode,
    pub message: String,
}

impl RecfstabError {
    /// Create a new error with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Root directory does not exist.
    pub fn root_not_found(path: &str) -> Self {
        Self::new(
            ErrorCode::RootNotFound,
            format!("root directory '{}' does not exist", path),
        )
    }

    /// Path is not a directory.
    pub fn not_a_directory(path: &str) -> Self {
        Self::new(
            ErrorCode::NotADirectory,
            format!("'{}' is not a directory", path),
        )
    }

    /// Failed to get current directory.
    pub fn current_dir_failed(source: std::io::Error) -> Self {
        Self::new(
            ErrorCode::CurrentDirFailed,
            format!("failed to determine current directory: {}", source),
        )
    }

    /// findmnt command not found.
    pub fn findmnt_not_found(source: std::io::Error) -> Self {
        Self::new(
            ErrorCode::FindmntNotFound,
            format!(
                "findmnt command not found (is util-linux installed?): {}",
                source
            ),
        )
    }

    /// findmnt command failed.
    pub fn findmnt_failed(stderr: &str) -> Self {
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr.trim().to_string()
        };
        Self::new(
            ErrorCode::FindmntFailed,
            format!("findmnt failed: {}", detail),
        )
    }

    /// No filesystems found under root.
    pub fn no_filesystems(root: &str) -> Self {
        Self::new(
            ErrorCode::NoFilesystems,
            format!(
                "no filesystems found under '{}' (make sure target filesystems are mounted)",
                root
            ),
        )
    }
}

impl fmt::Display for RecfstabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RecfstabError {}

/// Convenience type alias for Results using RecfstabError.
pub type Result<T> = std::result::Result<T, RecfstabError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_format() {
        assert_eq!(ErrorCode::RootNotFound.code(), "E001");
        assert_eq!(ErrorCode::NotADirectory.code(), "E002");
        assert_eq!(ErrorCode::CurrentDirFailed.code(), "E003");
        assert_eq!(ErrorCode::FindmntNotFound.code(), "E004");
        assert_eq!(ErrorCode::FindmntFailed.code(), "E005");
        assert_eq!(ErrorCode::NoFilesystems.code(), "E006");
    }

    #[test]
    fn test_error_display() {
        let err = RecfstabError::root_not_found("/mnt");
        let msg = err.to_string();
        assert!(msg.starts_with("E001:"), "Error was: {}", msg);
        assert!(msg.contains("/mnt"), "Error was: {}", msg);
    }

    #[test]
    fn test_error_not_a_directory() {
        let err = RecfstabError::not_a_directory("/etc/passwd");
        let msg = err.to_string();
        assert!(msg.starts_with("E002:"), "Error was: {}", msg);
        assert!(msg.contains("not a directory"), "Error was: {}", msg);
    }

    #[test]
    fn test_error_no_filesystems() {
        let err = RecfstabError::no_filesystems("/mnt/empty");
        let msg = err.to_string();
        assert!(msg.starts_with("E006:"), "Error was: {}", msg);
        assert!(msg.contains("no filesystems"), "Error was: {}", msg);
    }

    #[test]
    fn test_error_findmnt_failed_empty_stderr() {
        let err = RecfstabError::findmnt_failed("");
        let msg = err.to_string();
        assert!(msg.contains("unknown error"), "Error was: {}", msg);
    }

    #[test]
    fn test_error_findmnt_failed_with_stderr() {
        let err = RecfstabError::findmnt_failed("permission denied");
        let msg = err.to_string();
        assert!(msg.contains("permission denied"), "Error was: {}", msg);
    }

    #[test]
    fn test_all_error_codes_unique() {
        let codes = [
            ErrorCode::RootNotFound,
            ErrorCode::NotADirectory,
            ErrorCode::CurrentDirFailed,
            ErrorCode::FindmntNotFound,
            ErrorCode::FindmntFailed,
            ErrorCode::NoFilesystems,
        ];

        let mut seen = std::collections::HashSet::new();
        for code in codes {
            assert!(
                seen.insert(code.code()),
                "Duplicate error code: {}",
                code.code()
            );
        }
    }
}
