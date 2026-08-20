mod db;
mod file;
mod binarylog;
mod error;

pub use db::DB;
pub use error::DBError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_log_file_count() {
        let count = file::get_log_file_count("./test-dir").unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn check_log_threshold() {
        // test 1 KB greater than default 1 MB threshold
        let kb = 1024;
        let mb = kb * kb;
        assert!(!file::log_threshold_exceed(mb, kb as u32));
    }
}
