use lazy_static::lazy_static;
use regex::Regex;

/// Strips the file type suffix -  that is, everything after the last '.' - from the given file name.
pub(crate) fn strip_file_suffix(name: &str) -> String {
    lazy_static! {
        static ref FILE_SUFFIX: Regex = Regex::new(".[^.]*$").unwrap();
    }

    FILE_SUFFIX.replace(&name, "").to_string()
}
