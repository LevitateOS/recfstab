//! Integration tests for recfstab binary.
//!
//! These tests run the actual binary and verify behavior.

use std::process::Command;

/// Helper to run recfstab with given args
fn run_recfstab(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_recfstab"))
        .args(args)
        .output()
        .expect("Failed to execute recfstab")
}

// =============================================================================
// CLI Argument Tests
// =============================================================================

#[test]
fn test_help_flag() {
    let output = run_recfstab(&["--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check for key elements in help
    assert!(stdout.contains("fstab"), "Help should mention fstab");
    assert!(
        stdout.contains("--label") || stdout.contains("-L"),
        "Help should show label flag"
    );
    assert!(
        stdout.contains("<ROOT>") || stdout.contains("ROOT"),
        "Help should show ROOT argument"
    );
}

#[test]
fn test_version_flag() {
    let output = run_recfstab(&["--version"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("recfstab"));
}

#[test]
fn test_missing_root_argument() {
    let output = run_recfstab(&[]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap should complain about missing required argument
    assert!(
        stderr.contains("required") || stderr.contains("<ROOT>"),
        "stderr was: {}",
        stderr
    );
}

// =============================================================================
// Error Path Tests
// =============================================================================

#[test]
fn test_nonexistent_directory() {
    let output = run_recfstab(&["/nonexistent/path/12345"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show E001 error code
    assert!(
        stderr.contains("E001:"),
        "Expected E001, stderr was: {}",
        stderr
    );
    assert!(stderr.contains("does not exist"), "stderr was: {}", stderr);
}

#[test]
fn test_file_instead_of_directory() {
    let output = run_recfstab(&["/etc/passwd"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show E002 error code
    assert!(
        stderr.contains("E002:"),
        "Expected E002, stderr was: {}",
        stderr
    );
    assert!(stderr.contains("not a directory"), "stderr was: {}", stderr);
}

#[test]
fn test_empty_directory_no_mounts() {
    // /tmp should exist but have no real filesystem mounts under a random subdir
    let temp_dir = std::env::temp_dir().join("recfstab_test_empty");
    let _ = std::fs::create_dir_all(&temp_dir);

    let output = run_recfstab(&[temp_dir.to_str().unwrap()]);

    // Should fail because no mounts found
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show E006 error code
    assert!(
        stderr.contains("E006:"),
        "Expected E006, stderr was: {}",
        stderr
    );
    assert!(
        stderr.contains("no filesystems found"),
        "stderr was: {}",
        stderr
    );

    let _ = std::fs::remove_dir(&temp_dir);
}

// =============================================================================
// Exit Code Tests
// =============================================================================

#[test]
fn test_exit_code_success_on_help() {
    let output = run_recfstab(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_exit_code_failure_on_error() {
    let output = run_recfstab(&["/nonexistent"]);
    assert_ne!(output.status.code(), Some(0));
}

// =============================================================================
// Real Mount Tests (only if /proc/mounts exists)
// =============================================================================

#[test]
fn test_root_filesystem() {
    // Running against "/" should find at least the root mount
    // This only works if we're on a real Linux system with real mounts
    if !std::path::Path::new("/proc/mounts").exists() {
        return; // Skip on non-Linux
    }

    let output = run_recfstab(&["/"]);

    // In containers, "/" might not have real block device mounts (only overlayfs/tmpfs)
    // which get filtered out. So we only check format IF it succeeds with output.
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() && !stdout.trim().is_empty() {
            // If we got output, verify it looks like fstab
            for line in stdout.lines() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let fields: Vec<&str> = line.split_whitespace().collect();
                assert_eq!(fields.len(), 6, "fstab line should have 6 fields: {}", line);
            }
        }
    }
    // Failure is OK in containers - they may have no real block device mounts
}

#[test]
fn test_label_flag_parses() {
    // Test that -L flag is recognized (even if no labels exist)
    let output = run_recfstab(&["-L", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001, NOT with "unknown argument"
    assert!(
        stderr.contains("E001:"),
        "-L flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_long_label_flag() {
    // Test that --label flag works the same as -L
    let output = run_recfstab(&["--label", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001
    assert!(
        stderr.contains("E001:"),
        "--label flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_partuuid_flag_parses() {
    // Test that -p flag is recognized
    let output = run_recfstab(&["-p", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001, NOT with "unknown argument"
    assert!(
        stderr.contains("E001:"),
        "-p flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_long_partuuid_flag() {
    // Test that --partuuid flag works the same as -p
    let output = run_recfstab(&["--partuuid", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001
    assert!(
        stderr.contains("E001:"),
        "--partuuid flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_partlabel_flag_parses() {
    // Test that -t flag is recognized
    let output = run_recfstab(&["-t", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001, NOT with "unknown argument"
    assert!(
        stderr.contains("E001:"),
        "-t flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_long_partlabel_flag() {
    // Test that --partlabel flag works the same as -t
    let output = run_recfstab(&["--partlabel", "/nonexistent_path_12345"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with E001
    assert!(
        stderr.contains("E001:"),
        "--partlabel flag should be recognized, got: {}",
        stderr
    );
}

#[test]
fn test_conflicting_flags() {
    // Test that conflicting flags (-L and -p) produce an error
    let output = run_recfstab(&["-L", "-p", "/"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap should complain about conflicts
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "Should report conflicting flags, got: {}",
        stderr
    );
}

// =============================================================================
// Output Format Tests
// =============================================================================

#[test]
fn test_output_has_comment_lines() {
    if !std::path::Path::new("/proc/mounts").exists() {
        return;
    }

    let output = run_recfstab(&["/"]);
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Each entry should have a comment with the device
        assert!(
            stdout.contains("# /dev/") || stdout.contains("# UUID="),
            "Should have comment lines, got: {}",
            stdout
        );
    }
}

#[test]
fn test_output_is_valid_fstab_format() {
    if !std::path::Path::new("/proc/mounts").exists() {
        return;
    }

    let output = run_recfstab(&["/"]);
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Each non-comment line should have 6 whitespace-separated fields
            let fields: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 6, "fstab line should have 6 fields: {}", line);

            // Field 5 (dump) should be 0
            assert_eq!(fields[4], "0", "dump field should be 0");

            // Field 6 (pass) should be 0, 1, or 2
            let pass: u8 = fields[5].parse().expect("pass should be a number");
            assert!(pass <= 2, "pass should be 0, 1, or 2");
        }
    }
}

#[test]
fn test_pseudo_filesystems_excluded() {
    if !std::path::Path::new("/proc/mounts").exists() {
        return;
    }

    let output = run_recfstab(&["/"]);
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // These pseudo-filesystems should NEVER appear in output
        let forbidden = ["proc", "sysfs", "devpts", "tmpfs", "cgroup2", "devtmpfs"];

        for line in stdout.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 3 {
                let fstype = fields[2];
                assert!(
                    !forbidden.contains(&fstype),
                    "Pseudo-filesystem {} should be excluded, found in: {}",
                    fstype,
                    line
                );
            }
        }
    }
}
