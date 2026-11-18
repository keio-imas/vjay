use std::{fs, path::Path, process::Command};
use vjay_application::ports::PortResult;
pub(super) const LIMIT: u64 = 6 * 1024 * 1024 * 1024;
const RESERVE: u64 = 512 * 1024 * 1024;
pub(crate) fn available(root: &Path) -> PortResult<u64> {
    let output = Command::new("df")
        .arg("-Pk")
        .arg(root)
        .output()
        .map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .last()
        .and_then(|line| line.split_whitespace().nth(3))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.saturating_mul(1024))
        .ok_or_else(|| "Cannot determine video cache disk space".into())
}
pub(super) fn check(root: &Path, partial: Option<&Path>) -> PortResult<()> {
    if available(root)? < RESERVE {
        return Err("GPU video cache needs more free disk space".into());
    }
    if partial
        .and_then(|p| fs::metadata(p).ok())
        .is_some_and(|m| m.len() > LIMIT)
    {
        return Err("GPU video cache exceeds the 6 GiB per-clip limit".into());
    }
    Ok(())
}
