use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use crate::error::DBError;

pub fn open_log(path: impl AsRef<Path>, is_temp: bool) -> Result<File, std::io::Error> {
    let path = path.as_ref();

    let mut opts = OpenOptions::new();
    opts.read(true).create(true);

    if is_temp {
        opts.write(true).truncate(true);
    } else {
        opts.append(true);
    }

    opts.open(path)
}

pub fn get_log_file_count(dir: impl AsRef<Path>) -> Result<usize, DBError> {
    let path_ref = dir.as_ref();

    let read = std::fs::read_dir(path_ref)?;

    Ok(read
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .count())
}

pub fn log_threshold_exceed(max_bytes: u32, append_log_total: u32) -> bool {
    append_log_total > max_bytes
}

pub fn format_log_file_path(dir: impl AsRef<Path>, file_id: u32) -> PathBuf {
    let path_ref = dir.as_ref();
    path_ref.join(format!("{:04}.log", file_id))
}

pub fn remove_prev_log_files(dir: impl AsRef<Path>) -> Result<(), DBError> {
    let path_ref = dir.as_ref();

    for read in std::fs::read_dir(path_ref)? {
        let read = read?;
        let path = read.path();

        if !path.is_file() {
            continue;
        }

        let metadata = std::fs::metadata(&path)?;
        let mut perms = metadata.permissions();

        if perms.readonly() {
            perms.set_readonly(false);
            std::fs::set_permissions(&path, perms)?;

            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}

pub fn set_prevlog_readonly(file: impl AsRef<Path>) -> Result<(), DBError> {
    let path_ref = file.as_ref();

    let mut file_perm = std::fs::metadata(path_ref)?.permissions();
    file_perm.set_readonly(true);
    std::fs::set_permissions(path_ref, file_perm)?;

    Ok(())
}
