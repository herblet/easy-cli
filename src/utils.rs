use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use regex::Regex;

/// Strips the file type suffix -  that is, everything after the last '.' - from the given file name.
pub(crate) fn strip_file_suffix(name: &str) -> String {
    lazy_static! {
        static ref FILE_SUFFIX: Regex = Regex::new(".[^.]*$").unwrap();
    }

    FILE_SUFFIX.replace(&name, "").to_string()
}

const CACHE_DIR: &str = "easy-cli";

/// Compute the cache file path for a given scripts directory, stored under the
/// system cache directory (e.g. ~/Library/Caches/easy-cli on macOS).
pub(crate) fn cache_path_for(scripts_dir: &Path) -> Option<PathBuf> {
    let cache_dir = dirs::cache_dir()?.join(CACHE_DIR);
    std::fs::create_dir_all(&cache_dir).ok()?;
    let canonical = scripts_dir
        .canonicalize()
        .unwrap_or_else(|_| scripts_dir.to_path_buf());
    let sanitized = canonical
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    Some(cache_dir.join(sanitized))
}
