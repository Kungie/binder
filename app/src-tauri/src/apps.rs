//! "Open with": the apps Launch Services offers for a file, the default one first.

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppChoice {
    pub name: String,
    /// The `.app` bundle's path.
    pub path: String,
    pub default: bool,
}

fn choice(path: String, default: bool) -> AppChoice {
    let name = Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
    AppChoice { name, path, default }
}

/// The apps that can open `file`: the default first, then the rest by name, each once.
#[cfg(target_os = "macos")]
pub fn apps_for(file: &Path) -> Vec<AppChoice> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&file.to_string_lossy()));
    let ws = NSWorkspace::sharedWorkspace();
    let path_of = |u: &NSURL| u.path().map(|p| p.to_string());
    let default = ws.URLForApplicationToOpenURL(&url).and_then(|u| path_of(&u));
    let all: Vec<String> = ws.URLsForApplicationsToOpenURL(&url).iter().filter_map(|u| path_of(&u)).collect();
    order(default, all)
}

#[cfg(not(target_os = "macos"))]
pub fn apps_for(_file: &Path) -> Vec<AppChoice> {
    vec![]
}

fn order(default: Option<String>, all: Vec<String>) -> Vec<AppChoice> {
    let mut rest: Vec<AppChoice> = all.into_iter().filter(|p| Some(p) != default.as_ref()).map(|p| choice(p, false)).collect();
    rest.sort_by_key(|a| a.name.to_lowercase());
    rest.dedup_by(|a, b| a.path == b.path);
    default.map(|d| choice(d, true)).into_iter().chain(rest).collect()
}

/// An app Binder may launch: an existing `.app` bundle.
pub fn is_app(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "app") && path.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_app_comes_first() {
        let got = order(Some("/Applications/Zed.app".into()), vec!["/System/Applications/TextEdit.app".into(), "/Applications/Zed.app".into(), "/Applications/BBEdit.app".into()]);
        let names: Vec<&str> = got.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Zed", "BBEdit", "TextEdit"]);
        assert!(got[0].default && !got[1].default);
        assert_eq!(order(None, vec!["/A/b.app".into()])[0].default, false);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apps_for_a_text_file_includes_textedit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();
        let apps = apps_for(&file);
        assert!(apps.iter().any(|a| a.name == "TextEdit"), "{apps:?}");
        assert_eq!(apps.iter().filter(|a| a.default).count(), 1);
        assert!(apps[0].default);
        assert!(is_app(Path::new("/System/Applications/TextEdit.app")));
        assert!(!is_app(&file));
    }
}
