#[cfg(windows)]
pub fn allocated_size(path: impl AsRef<std::path::Path>) -> std::io::Result<u64> {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::Win32::Foundation::GetLastError;
    use windows::Win32::Storage::FileSystem::GetCompressedFileSizeW;
    use windows::core::PCWSTR;

    const INVALID_FILE_SIZE: u32 = u32::MAX;

    /// Converts a path into a null-terminated UTF-16 string for Windows APIs.
    ///
    /// Many Windows APIs expect paths as wide strings (`PCWSTR`), which are UTF-16
    /// encoded and terminated with a trailing `0`.
    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let wide = to_wide(path.as_ref());

    let mut high: u32 = 0;

    // Clear last OS error before calling, because INVALID_FILE_SIZE can also be
    // a valid low 32-bit value for very large files.
    unsafe {
        windows::Win32::Foundation::SetLastError(windows::Win32::Foundation::WIN32_ERROR(0));
    }

    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high)) };

    if low == INVALID_FILE_SIZE {
        let err = unsafe { GetLastError() };

        if err.0 != 0 {
            return Err(io::Error::from_raw_os_error(err.0 as i32));
        }
    }

    Ok(((high as u64) << 32) | low as u64)
}

#[cfg(unix)]
pub fn allocated_size(path: impl AsRef<std::path::Path>) -> std::io::Result<u64> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;

    Ok(metadata.blocks() * 512)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn allocated_size_returns_size_for_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, b"hello world").unwrap();

        let size = allocated_size(&file_path).unwrap();

        assert!(size > 0);
    }

    #[test]
    fn allocated_size_returns_error_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("missing.txt");

        let result = allocated_size(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn allocated_size_is_at_least_logical_size_for_normal_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.bin");

        let data = vec![1u8; 10_000];
        fs::write(&file_path, &data).unwrap();

        let logical_size = fs::metadata(&file_path).unwrap().len();
        let allocated = allocated_size(&file_path).unwrap();

        assert!(allocated >= logical_size);
    }

    #[test]
    fn allocated_size_is_reasonable_for_larger_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large.bin");

        let data = vec![1u8; 128 * 1024];
        fs::write(&file_path, &data).unwrap();

        let logical_size = fs::metadata(&file_path).unwrap().len();
        let allocated = allocated_size(&file_path).unwrap();

        assert!(allocated >= logical_size);
        assert!(allocated < logical_size + 1024 * 1024);
    }
}
