use std::path::Path;
use std::fs;
use crate::diff::{FileDiff, ChangeType};

pub fn apply_update(project: &Path, diff: Vec<FileDiff>) {
    for file in diff {
        match file.change_type {
            ChangeType::Add => {
                let path = project.join(&file.path);
                println!("Create {} ({} bytes)", path.display(), file.change_contents.len());
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, file.change_contents).unwrap();
            },
            ChangeType::Delete => {
                let path = project.join(&file.path);
                println!("Delete {}", path.display());
                fs::remove_file(path).unwrap();
            },
            ChangeType::Modify => {
                let path = project.join(&file.path);
                println!("Modify {} ({} bytes)", path.display(), file.change_contents.len());
                fs::write(path, file.change_contents).unwrap();
            },
        }
    }
}