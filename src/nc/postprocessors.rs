use std::fs;
use std::path::PathBuf;

pub fn find_postprocessor(name: &str) -> Option<PathBuf> {
    let search_dirs = postprocessor_dirs();
    for dir in search_dirs {
        let path = dir.join(format!("{}.lua", name));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn list_postprocessors() -> Vec<String> {
    let mut names = Vec::new();
    for dir in postprocessor_dirs() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_stem()
                    && path.extension().is_some_and(|e| e == "lua")
                    && name != "base"
                {
                    names.push(name.to_string_lossy().into_owned());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn postprocessor_dirs() -> Vec<PathBuf> {
    // Look in dir postprocessors next to the binary
    let mut dirs = vec![PathBuf::from("postprocessors")];

    // Also look in user postprocessors (i.e. ~/.config/chipmunk/postprocessors)
    if let Some(config) = dirs::config_dir() {
        dirs.push(config.join("chipmunk/postprocessors"));
    }

    dirs
}
