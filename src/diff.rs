use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::io::BufReader;
use std::io::prelude::*;
use std::{fs, u8};

#[derive(Clone)]
pub enum ChangeType {
    Add,
    Delete,
    Modify,
}

#[derive(Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub change_contents: Vec<u8>,
}

fn _file_diff(f1: &Path, f2: &Path, prefix1: &Path, prefix2: &Path) -> Option<FileDiff> {
    if f1.exists() && !f2.exists() {
        let contents = fs::read(f1);
        if contents.is_err() {
            return None;
        }

        Some(FileDiff {
            path: f1.strip_prefix(prefix1).unwrap().to_path_buf(),
            change_type: ChangeType::Delete,
            change_contents: contents.unwrap(),
        })
    } else if !f1.exists() && f2.exists() {
        let contents = fs::read(f2);
        if contents.is_err() {
            return None;
        }

        Some(FileDiff {
            path: f2.strip_prefix(prefix1).unwrap().to_path_buf(),
            change_type: ChangeType::Add,
            change_contents: contents.unwrap(),
        })
    } else if f1.exists() && f2.exists() {
        let cnt1 = fs::read(f1).unwrap();
        let cnt2 = fs::read(f2).unwrap();

        if cnt1 == cnt2 {
            None
        } else {
            Some(FileDiff {
                path: f2.strip_prefix(prefix2).unwrap().to_path_buf(),
                change_type: ChangeType::Modify,
                change_contents: cnt2,
            })
        }
    } else {
        None
    }
}

// This funcion scans two projects and returns a list of `FileDiff` structs

fn _recursive_diff(dir1: &Path, base_prefix1: &Path, base_prefix2: &Path, ignore_dirs: &Vec<OsString>) -> Result<Vec<FileDiff>, String> {
    let mut diffs = Vec::<FileDiff>::new();
    let iterator = fs::read_dir(dir1);
    if iterator.is_err() {
        return Err("Directory does not exist".to_string());
    }

    for entry in iterator.unwrap() {
        if !entry.is_ok() {
            continue;
        }

        let dirent = entry.unwrap();
        let path = dirent.path();

        if path.is_dir() && path.file_name().unwrap().to_str().unwrap().starts_with('.') { // ignore hidden files
            continue
        }

        let rel_path = path.strip_prefix(base_prefix1).unwrap();
        if path.is_dir() {
            let is_in = ignore_dirs.iter().any(|x| *x == OsString::from(rel_path.file_name().unwrap()));
            if is_in {
                continue;
            }
            let sub_diffs = _recursive_diff(&path, base_prefix1, base_prefix2, ignore_dirs);
            if sub_diffs.is_err() {
                return Err(sub_diffs.err().unwrap());
            }

            diffs.extend(sub_diffs.unwrap());
        }
        else {
            let diff = _file_diff(&path, base_prefix2.join(rel_path).as_path(), base_prefix1, base_prefix2);
            if diff.is_some() {
                diffs.push(diff.unwrap());
            }
        }
    }

    Ok(diffs)
}

// This one returns files that are present in dir2 but not dir1. first func iterates over dir1, so obviously it wouldn't find those files
fn _recursive_diff2(dir2: &Path, base_prefix1: &Path, base_prefix2: &Path, ignore_dirs: &Vec<OsString>) -> Result<Vec<FileDiff>, String> {
    let mut diffs = Vec::<FileDiff>::new();
    let iterator = fs::read_dir(dir2);
    if iterator.is_err() {
        return Err(iterator.err().unwrap().to_string());
    }

    for entry in iterator.unwrap() {
        if !entry.is_ok() {
            continue;
        }

        let dirent = entry.unwrap();
        let path = dirent.path();
        let rel_path = path.strip_prefix(base_prefix2).unwrap();

        if path.is_dir() && path.file_name().unwrap().to_str().unwrap().starts_with('.') { // ignore hidden files
            continue
        }

        // check for ignored dirs
        else if path.is_dir() {
            let is_in = ignore_dirs.iter().any(|x| *x == rel_path.file_name().unwrap());
            if is_in {
                continue;
            }
            let subd = _recursive_diff2(&path, base_prefix1, base_prefix2, ignore_dirs);
            if subd.is_err() {
                return Err(subd.err().unwrap());
            }
            diffs.extend(subd.unwrap());

        }

        else if !base_prefix1.join(rel_path).exists() {
            let diff = _file_diff(base_prefix1.join(rel_path).as_path(), &path, base_prefix2, base_prefix1);
            if diff.is_some() {
                diffs.push(diff.unwrap());
            }
            else {
                eprintln!("Could not diff {}", path.display());
            }
        }
    }

    Ok(diffs)
}

pub fn gen_diff(p1: &Path, p2: &Path, ignore_dirs: &Vec<OsString>) -> Result<Vec<FileDiff>, String> {
    if !p1.is_dir() || !p2.is_dir() {
        return Err("Both paths must be directories".to_string());
    }

    let diffs = _recursive_diff(p1, &p1, &p2, &ignore_dirs);
    if diffs.is_err() {
        return Err(diffs.err().unwrap());
    }

    let mut diffs = diffs.unwrap();
    diffs.extend(_recursive_diff2(p2, &p1, &p2, &ignore_dirs).unwrap());

    Ok(diffs)
}

pub fn save_diff(diff: Vec<FileDiff>, p: &Path) -> Result<(), String> {
    let file = fs::File::create(p);
    if file.is_err() {
        return Err("Could not create file".to_string());
    }

    let mut file = file.unwrap();

    for f in diff {
        let mut line = Vec::<u8>::new();
        match f.change_type {
            ChangeType::Add => {
                line.push('A' as u8);
            }
            ChangeType::Delete => {
                line.push('D' as u8);
            }
            ChangeType::Modify => {
                line.push('M' as u8);
            }
        }
        line.extend(f.path.to_str().unwrap().as_bytes());
        line.extend(hex::decode("d6").unwrap());

        let len: u32 = f.change_contents.len() as u32;
        let len = len.to_le_bytes();
        line.extend(&len);

        line.extend(f.change_contents);

        if file.write_all(line.as_slice()).is_err() {
            eprintln!("Could not write to file");
            ()
        }
    }

    Ok(())
}

pub fn load_diff(p: &Path) -> Result<Vec<FileDiff>, String> {
    let mut diffs = Vec::<FileDiff>::new();
    let file = fs::File::open(p);


    if file.is_err() {
        return Err("Could not open file".to_string());
    }

    let file = file.unwrap();
    let mut reader = BufReader::new(file);

    loop {
        let mut buf: [u8; 1] = [0; 1];
        let mut buf_vec = Vec::<u8>::new();

        buf_vec.clear();

        // Get the mode and the relative path
        let res = reader.read(&mut buf);
        if res.is_err() {
            break;
        }

        let mode = buf[0];

        let ctype = match mode as char {
            'A' => ChangeType::Add,
            'D' => ChangeType::Delete,
            'M' => ChangeType::Modify,
            _ => break, // assume its over
        };

        let mut path_buf = Vec::<u8>::new();
        if reader.read_until(0xd6, &mut path_buf).is_err() {
            return Err("Could not read path".to_string());
        }

        // remove 0xd6
        path_buf.pop();

        // buf_big now contains the path
        let path = String::from_utf8(path_buf).unwrap();

        let mut buf_size: [u8; 4] = [0; 4];
        if reader.read(&mut buf_size).is_err() {
            break;
        }

        let size = u32::from_le_bytes(buf_size);
        let mut buf_contents = vec![0u8; size as usize];

        if reader.read_exact(&mut buf_contents).is_err() {
            break;
        }

        diffs.push(FileDiff {
            path: PathBuf::from(path),
            change_type: ctype,
            change_contents: buf_contents
        });
    }
    Ok(diffs)
}