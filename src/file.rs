use std::{
    fs::{File, OpenOptions}, path::{Path, PathBuf},
};

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

pub fn get_log_file_count(dir_name: Option<&str>) -> Result<usize, Box<dyn std::error::Error>> {
    let dir = dir_name.unwrap_or("logs");

    let read = std::fs::read_dir(dir)?;

    Ok(read
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .count())
}

pub fn check_log_threshold(append_log_total: u32) -> bool {
    const MAX_BYTES: u32 = 70; // 70 bytes only to test multi log files
    append_log_total >= MAX_BYTES
}

pub fn format_log_file_path(dir: &Path, file_id: u32) -> PathBuf {
    dir.join(format!("{:04}.log", file_id))
}
