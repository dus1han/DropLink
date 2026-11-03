//! Turns user-selected paths into the flat item list sent in an offer.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{Error, Result};

pub const MAX_ITEMS: usize = 200_000;

#[derive(Debug, Clone)]
pub struct SourceItem {
    /// `/`-separated path relative to the transfer root.
    pub rel_path: String,
    pub source: PathBuf,
    pub size: u64,
    pub mtime: Option<i64>,
    pub is_dir: bool,
}

pub struct Collected {
    pub items: Vec<SourceItem>,
    pub label: String,
    pub has_dirs: bool,
}

fn mtime_ms(meta: &fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

fn file_name(p: &Path) -> Result<String> {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Invalid(format!("{} has no file name", p.display())))
}

/// Walks the selection. Symbolic links are skipped rather than followed so a
/// link cannot pull in files outside the selected folder or loop forever.
pub fn collect(paths: &[PathBuf]) -> Result<Collected> {
    if paths.is_empty() {
        return Err(Error::Invalid("nothing selected".into()));
    }
    let mut items = Vec::new();
    let mut top_names = HashSet::new();
    let mut has_dirs = false;

    for path in paths {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| Error::Invalid(format!("cannot read {}: {e}", path.display())))?;
        let meta = if meta.file_type().is_symlink() { fs::metadata(path)? } else { meta };
        // Two selected items with the same name (from different folders)
        // would collide at the destination, so disambiguate here.
        let base = file_name(path)?;
        let mut name = base.clone();
        let mut n = 2;
        while !top_names.insert(name.to_lowercase()) {
            name = match base.rfind('.') {
                Some(i) if i > 0 && !meta.is_dir() => format!("{} ({n}){}", &base[..i], &base[i..]),
                _ => format!("{base} ({n})"),
            };
            n += 1;
        }

        if meta.is_dir() {
            has_dirs = true;
            items.push(SourceItem {
                rel_path: name.clone(),
                source: path.clone(),
                size: 0,
                mtime: mtime_ms(&meta),
                is_dir: true,
            });
            walk(path, &name, &mut items)?;
        } else if meta.is_file() {
            items.push(SourceItem {
                rel_path: name,
                source: path.clone(),
                size: meta.len(),
                mtime: mtime_ms(&meta),
                is_dir: false,
            });
        }
        if items.len() > MAX_ITEMS {
            return Err(Error::Invalid(format!("too many files (limit {MAX_ITEMS})")));
        }
    }

    let files = items.iter().filter(|i| !i.is_dir).count();
    let label = if paths.len() == 1 {
        file_name(&paths[0])?
    } else if has_dirs {
        format!("{} items", paths.len())
    } else {
        format!("{files} files")
    };
    Ok(Collected { items, label, has_dirs })
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<SourceItem>) -> Result<()> {
    let mut stack = vec![(dir.to_path_buf(), rel.to_string())];
    while let Some((dir, rel)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let child_rel = format!("{rel}/{name}");
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("skipping {}: {e}", entry.path().display());
                    continue;
                }
            };
            if ft.is_dir() {
                out.push(SourceItem {
                    rel_path: child_rel.clone(),
                    source: entry.path(),
                    size: 0,
                    mtime: mtime_ms(&meta),
                    is_dir: true,
                });
                stack.push((entry.path(), child_rel));
            } else if ft.is_file() {
                out.push(SourceItem {
                    rel_path: child_rel,
                    source: entry.path(),
                    size: meta.len(),
                    mtime: mtime_ms(&meta),
                    is_dir: false,
                });
            }
            if out.len() > MAX_ITEMS {
                return Err(Error::Invalid(format!("too many files (limit {MAX_ITEMS})")));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_folders_and_disambiguates_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir_all(a.join("photos/empty")).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("photos/1.jpg"), b"one").unwrap();
        fs::write(a.join("notes.txt"), b"a").unwrap();
        fs::write(b.join("notes.txt"), b"bb").unwrap();

        let c = collect(&[a.join("photos"), a.join("notes.txt"), b.join("notes.txt")]).unwrap();
        let paths: Vec<_> = c.items.iter().map(|i| i.rel_path.as_str()).collect();
        assert_eq!(paths, vec!["photos", "photos/1.jpg", "photos/empty", "notes.txt", "notes (2).txt"]);
        assert!(c.has_dirs);
        assert_eq!(c.label, "3 items");
        assert_eq!(c.items.iter().map(|i| i.size).sum::<u64>(), 6);
    }
}
