use std::{
    fs::{File, OpenOptions},
    path::Path,
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
