pub mod db;
pub mod file;
pub mod binarylog;
pub mod error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_log_file_count() {
        let count = file::get_log_file_count(Some("test-dir")).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_check_log_threshold() {
        let test_byte = 1024; // test 1KB greater than 70 byte threshold
        assert!(file::check_log_threshold(test_byte));
    }
}
