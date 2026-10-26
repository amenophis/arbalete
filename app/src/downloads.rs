//! Downloads: where they are saved, and what the sidebar shows of them.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// CEF's id, unique for the session.
    pub id: u32,
    /// The file's name, as saved.
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub received: i64,
    /// 0 while the size is unknown.
    pub total: i64,
    /// Bytes per second.
    pub speed: i64,
    pub state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    InProgress,
    Complete,
    Canceled,
    Failed,
}

impl Download {
    /// A download about to be saved at `path`.
    pub fn new(id: u32, path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            id,
            name,
            path,
            received: 0,
            total: 0,
            speed: 0,
            state: State::InProgress,
        }
    }
}

/// A path in `dir` for a file named `name` that `taken` does not refuse: `report.pdf`, then
/// `report (1).pdf`, `report (2).pdf`… A name holding a path keeps only its last part.
pub fn unique_path(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let name = Path::new(name)
        .file_name()
        .map_or_else(|| "download".into(), |name| name.to_string_lossy());
    let path = dir.join(name.as_ref());
    if !taken(&path) {
        return path;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        // A leading dot starts a hidden file's name, not an extension.
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name.as_ref(), String::new()),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){extension}")))
        .find(|path| !taken(path))
        .expect("a free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(names: &[&str]) -> impl Fn(&Path) -> bool {
        let paths: Vec<PathBuf> = names
            .iter()
            .map(|name| Path::new("/d").join(name))
            .collect();
        move |path| paths.iter().any(|taken| taken == path)
    }

    #[test]
    fn keeps_a_free_name() {
        let path = unique_path(Path::new("/d"), "report.pdf", taken(&[]));
        assert_eq!(path, Path::new("/d/report.pdf"));
    }

    #[test]
    fn numbers_a_taken_name_before_its_extension() {
        let path = unique_path(
            Path::new("/d"),
            "report.pdf",
            taken(&["report.pdf", "report (1).pdf"]),
        );
        assert_eq!(path, Path::new("/d/report (2).pdf"));
    }

    #[test]
    fn numbers_names_without_an_extension_and_hidden_files_at_the_end() {
        let path = unique_path(Path::new("/d"), "notes", taken(&["notes"]));
        assert_eq!(path, Path::new("/d/notes (1)"));
        let path = unique_path(Path::new("/d"), ".env", taken(&[".env"]));
        assert_eq!(path, Path::new("/d/.env (1)"));
    }

    #[test]
    fn keeps_only_the_last_part_of_a_name() {
        let path = unique_path(Path::new("/d"), "../etc/passwd", taken(&[]));
        assert_eq!(path, Path::new("/d/passwd"));
        let path = unique_path(Path::new("/d"), "", taken(&[]));
        assert_eq!(path, Path::new("/d/download"));
    }

    #[test]
    fn names_a_download_after_its_file() {
        let download = Download::new(7, PathBuf::from("/d/report (1).pdf"));
        assert_eq!(download.name, "report (1).pdf");
        assert_eq!(download.state, State::InProgress);
    }
}
