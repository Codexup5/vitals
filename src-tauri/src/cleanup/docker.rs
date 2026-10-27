//! Docker's own account of what it could free, read from `docker system df` and `docker images`.

use std::time::Duration;

const DOCKER_TIMEOUT: Duration = Duration::from_secs(15);

/// `1.234GB`, `512MB`, `12.5kB`, `0B` as Docker prints them (decimal units) into bytes.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text.find(|c: char| c.is_ascii_alphabetic())?;
    let (number, unit) = text.split_at(split);
    let number: f64 = number.trim().parse().ok()?;
    let factor = match unit.trim() {
        "B" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative size well inside u64"
    )]
    Some((number * factor).round() as u64)
}

/// The reclaimable bytes of one `docker system df --format "{{json .}}"` line of the given type.
pub fn reclaimable(lines: &str, kind: &str) -> Option<u64> {
    lines.lines().find_map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).ok()?;
        if row.get("Type")?.as_str()? != kind {
            return None;
        }
        let text = row.get("Reclaimable")?.as_str()?;
        parse_size(text.split_whitespace().next()?)
    })
}

/// Runs docker with a deadline: a stopped Docker Desktop must not hang the scan.
pub fn run(args: &[&str]) -> Option<String> {
    super::process::run("docker", args, DOCKER_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_sizes() {
        assert_eq!(parse_size("1.5GB"), Some(1_500_000_000));
        assert_eq!(parse_size("512MB"), Some(512_000_000));
        assert_eq!(parse_size("12.5kB"), Some(12_500));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("lots"), None);
    }

    #[test]
    fn reads_the_reclaimable_part_of_a_type() {
        let lines = "{\"Type\":\"Images\",\"Reclaimable\":\"6.2GB (40%)\"}\n{\"Type\":\"Build Cache\",\"Reclaimable\":\"0B\"}";
        assert_eq!(reclaimable(lines, "Images"), Some(6_200_000_000));
        assert_eq!(reclaimable(lines, "Build Cache"), Some(0));
        assert_eq!(reclaimable(lines, "Volumes"), None);
    }
}
