use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "unknown",
    }
}

pub fn default_device_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().trim().to_string();
    let host = host.trim_end_matches(".local").to_string();
    if host.is_empty() {
        "DropLink device".into()
    } else {
        host
    }
}

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Characters Windows refuses in file names. Replacing them on every platform
/// keeps names identical on both ends of a transfer.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\\'];
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Converts a `/`-separated relative path received from a peer into a safe
/// local relative path. Rejects anything that could escape the destination
/// directory (absolute paths, `..`, empty components).
pub fn safe_relative_path(raw: &str) -> Result<PathBuf> {
    if raw.is_empty() || raw.len() > 4096 {
        return Err(Error::Protocol("invalid path length".into()));
    }
    if raw.starts_with('/') || raw.contains('\0') {
        return Err(Error::Protocol(format!("unsafe path: {raw}")));
    }
    let mut out = PathBuf::new();
    for part in raw.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(Error::Protocol(format!("unsafe path: {raw}")));
        }
        out.push(sanitize_component(part));
    }
    Ok(out)
}

pub fn sanitize_component(part: &str) -> String {
    let mut s: String = part
        .chars()
        .map(|c| if FORBIDDEN.contains(&c) || c.is_control() { '_' } else { c })
        .collect();
    // Windows silently strips trailing dots/spaces, which would let two
    // different names collide.
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() {
        s.push('_');
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    s
}

/// Returns `dir/name`, or `dir/name (2).ext`, `dir/name (3).ext`… if taken.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("unbounded iterator")
}

pub fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Human readable fingerprint: `AB12 CD34 EF56 …` (first 16 bytes).
pub fn fingerprint_display(id: &str) -> String {
    id.to_uppercase()
        .as_bytes()
        .chunks(4)
        .take(8)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_local_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                // RFC 6598 shared space, used by Tailscale and carrier NAT.
                || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local_ip(std::net::IpAddr::V4(v4));
            }
            let seg = v6.segments()[0];
            v6.is_loopback() || (seg & 0xfe00) == 0xfc00 || (seg & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        for bad in ["../x", "a/../../b", "/etc/passwd", "a//b", "", "./a", "a/."] {
            assert!(safe_relative_path(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn sanitizes_windows_names() {
        let p = safe_relative_path("dir/CON.txt").unwrap();
        assert_eq!(p, PathBuf::from("dir").join("_CON.txt"));
        let p = safe_relative_path("a:b/c?d. ").unwrap();
        assert_eq!(p, PathBuf::from("a_b").join("c_d"));
        // A drive prefix becomes an ordinary name, never an absolute path.
        let p = safe_relative_path("C:/Windows").unwrap();
        assert!(p.is_relative());
        assert_eq!(p, PathBuf::from("C_").join("Windows"));
    }

    #[test]
    fn unique_path_appends_counter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("a (2).txt"), b"x").unwrap();
        assert_eq!(unique_path(dir.path(), "a.txt"), dir.path().join("a (3).txt"));
        assert_eq!(unique_path(dir.path(), "b"), dir.path().join("b"));
    }

    #[test]
    fn local_ip_classification() {
        assert!(is_local_ip("192.168.1.4".parse().unwrap()));
        assert!(is_local_ip("100.101.1.4".parse().unwrap()));
        assert!(is_local_ip("fe80::1".parse().unwrap()));
        assert!(!is_local_ip("8.8.8.8".parse().unwrap()));
    }
}
