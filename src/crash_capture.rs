#[cfg(windows)]
use crate::crash_ui;
#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
#[cfg(windows)]
use std::fs::{self, OpenOptions};
#[cfg(windows)]
use std::io::Write;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
use time::format_description::well_known::Rfc3339;
#[cfg(windows)]
use time::OffsetDateTime;

#[cfg(windows)]
const APP_DIR_NAME: &str = "aura";
#[cfg(windows)]
const CRASH_DUMP_FILENAME: &str = "aura-crash.dmp";
#[cfg(windows)]
const CRASH_TEXT_FILENAME: &str = "aura-crash.txt";

#[cfg(not(any(windows, unix)))]
pub fn install() -> Result<()> {
    Ok(())
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use anyhow::Context;
    use std::fs::{self, OpenOptions};
    use std::os::fd::IntoRawFd;
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    const CRASH_TEXT_FILENAME: &str = "aura-crash.txt";
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static CRASH_FD: AtomicI32 = AtomicI32::new(-1);

    pub fn install() -> Result<()> {
        if INSTALLED.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let state_dir = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .context("failed to resolve Unix state directory")?
            .join("aura");
        fs::create_dir_all(&state_dir)
            .with_context(|| format!("failed to create crash directory {}", state_dir.display()))?;
        let crash_path = state_dir.join(CRASH_TEXT_FILENAME);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .with_context(|| format!("failed to open crash log {}", crash_path.display()))?;
        CRASH_FD.store(file.into_raw_fd(), Ordering::SeqCst);

        for signal in [
            libc::SIGSEGV,
            libc::SIGABRT,
            libc::SIGBUS,
            libc::SIGILL,
            libc::SIGFPE,
        ] {
            install_signal_handler(signal)?;
        }
        Ok(())
    }

    fn install_signal_handler(signal: libc::c_int) -> Result<()> {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = fatal_signal_handler as *const () as usize;
        action.sa_flags = libc::SA_RESETHAND;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error()).with_context(|| {
                    format!("failed to install fatal signal handler for signal {signal}")
                });
            }
        }
        Ok(())
    }

    unsafe extern "C" fn fatal_signal_handler(signal: libc::c_int) {
        let mut message = [0_u8; 64];
        let prefix = b"Aura received fatal signal ";
        message[..prefix.len()].copy_from_slice(prefix);
        let mut length = prefix.len();
        length += write_decimal(signal, &mut message[length..]);
        message[length] = b'\n';
        length += 1;

        unsafe {
            libc::write(libc::STDERR_FILENO, message.as_ptr().cast(), length);
            let fd = CRASH_FD.load(Ordering::Relaxed);
            if fd >= 0 {
                libc::write(fd, message.as_ptr().cast(), length);
                libc::fsync(fd);
            }
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
            libc::_exit(128 + signal);
        }
    }

    fn write_decimal(value: libc::c_int, output: &mut [u8]) -> usize {
        let mut value = value.max(0) as u32;
        let mut reverse = [0_u8; 10];
        let mut count = 0;
        loop {
            reverse[count] = b'0' + (value % 10) as u8;
            count += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        for index in 0..count {
            output[index] = reverse[count - index - 1];
        }
        count
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn formats_signal_number_without_allocating() {
            let mut output = [0_u8; 10];
            let length = write_decimal(11, &mut output);
            assert_eq!(&output[..length], b"11");
        }
    }
}

#[cfg(unix)]
pub use unix_impl::install;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Diagnostics::Debug::{
        MiniDumpNormal, MiniDumpWithDataSegs, MiniDumpWithThreadInfo, MiniDumpWriteDump,
        SetUnhandledExceptionFilter, EXCEPTION_POINTERS, MINIDUMP_EXCEPTION_INFORMATION,
        MINIDUMP_TYPE,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
    };

    #[derive(Clone)]
    struct CrashPaths {
        dump_path: PathBuf,
        text_path: PathBuf,
    }

    static PATHS: OnceLock<CrashPaths> = OnceLock::new();
    static FILTER_INSTALLED: AtomicBool = AtomicBool::new(false);
    static HANDLING_CRASH: AtomicBool = AtomicBool::new(false);

    pub fn install() -> Result<()> {
        let paths = resolve_paths()?;
        let _ = PATHS.set(paths);

        if FILTER_INSTALLED.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        unsafe {
            SetUnhandledExceptionFilter(Some(unhandled_exception_filter));
        }
        Ok(())
    }

    unsafe extern "system" fn unhandled_exception_filter(
        exception_info: *const EXCEPTION_POINTERS,
    ) -> i32 {
        if HANDLING_CRASH.swap(true, Ordering::SeqCst) {
            return 1;
        }

        let (exception_code, exception_address) = read_exception_details(exception_info);
        let thread_id = unsafe { GetCurrentThreadId() };
        let timestamp = crash_timestamp();

        if let Some(paths) = PATHS.get() {
            let dump_result = write_dump(&paths.dump_path, exception_info);
            let text_result = write_crash_text(
                &paths.text_path,
                &timestamp,
                exception_code,
                exception_address,
                thread_id,
                dump_result.is_ok(),
                dump_result.as_ref().err().map(|error| format!("{error:#}")),
            );

            let _ = writeln!(
                std::io::stderr(),
                "native crash captured: dump={} text={} code=0x{:08X} addr=0x{:X}",
                paths.dump_path.display(),
                paths.text_path.display(),
                exception_code,
                exception_address,
            );
            if let Err(error) = text_result {
                let _ = writeln!(
                    std::io::stderr(),
                    "failed to write crash text log: {error:#}"
                );
            }
            crash_ui::show_native_crash_dialog(
                exception_code,
                exception_address,
                Some(paths.text_path.as_path()),
            );
        } else {
            let _ = writeln!(
                std::io::stderr(),
                "native crash captured without crash paths: code=0x{:08X} addr=0x{:X}",
                exception_code,
                exception_address,
            );
            crash_ui::show_native_crash_dialog(exception_code, exception_address, None);
        }

        HANDLING_CRASH.store(false, Ordering::SeqCst);
        1
    }

    fn write_dump(path: &Path, exception_info: *const EXCEPTION_POINTERS) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("failed to open crash dump file {}", path.display()))?;

        let exception = MINIDUMP_EXCEPTION_INFORMATION {
            ThreadId: unsafe { GetCurrentThreadId() },
            ExceptionPointers: exception_info as *mut EXCEPTION_POINTERS,
            ClientPointers: 0,
        };
        let dump_flags: MINIDUMP_TYPE =
            MiniDumpNormal | MiniDumpWithDataSegs | MiniDumpWithThreadInfo;

        let ok = unsafe {
            MiniDumpWriteDump(
                GetCurrentProcess(),
                GetCurrentProcessId(),
                file.as_raw_handle() as HANDLE,
                dump_flags,
                &exception,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if ok == 0 {
            anyhow::bail!("MiniDumpWriteDump returned failure");
        }
        Ok(())
    }

    fn write_crash_text(
        path: &Path,
        timestamp: &str,
        exception_code: u32,
        exception_address: usize,
        thread_id: u32,
        dump_written: bool,
        dump_error: Option<String>,
    ) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("failed to open crash text file {}", path.display()))?;
        writeln!(file, "timestamp_utc={timestamp}")?;
        writeln!(file, "exception_code=0x{exception_code:08X}")?;
        writeln!(file, "exception_address=0x{exception_address:X}")?;
        writeln!(file, "thread_id={thread_id}")?;
        writeln!(file, "dump_written={dump_written}")?;
        if let Some(dump_error) = dump_error {
            writeln!(file, "dump_error={dump_error}")?;
        }
        file.flush()?;
        Ok(())
    }

    fn read_exception_details(exception_info: *const EXCEPTION_POINTERS) -> (u32, usize) {
        if exception_info.is_null() {
            return (0, 0);
        }

        let record = unsafe { (*exception_info).ExceptionRecord };
        if record.is_null() {
            return (0, 0);
        }

        unsafe {
            (
                (*record).ExceptionCode as u32,
                (*record).ExceptionAddress as usize,
            )
        }
    }

    fn resolve_paths() -> Result<CrashPaths> {
        let Some(local_data_dir) = dirs::data_local_dir() else {
            anyhow::bail!("failed to resolve local data directory");
        };
        let app_dir = local_data_dir.join(APP_DIR_NAME);
        fs::create_dir_all(&app_dir)
            .with_context(|| format!("failed to create crash directory {}", app_dir.display()))?;
        Ok(CrashPaths {
            dump_path: app_dir.join(CRASH_DUMP_FILENAME),
            text_path: app_dir.join(CRASH_TEXT_FILENAME),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn resolves_expected_crash_file_names() {
            let root = PathBuf::from("C:\\Users\\user\\AppData\\Local");
            let app_dir = root.join("aura");
            assert_eq!(
                app_dir.join(CRASH_DUMP_FILENAME).file_name().unwrap(),
                "aura-crash.dmp"
            );
            assert_eq!(
                app_dir.join(CRASH_TEXT_FILENAME).file_name().unwrap(),
                "aura-crash.txt"
            );
        }
    }
}

#[cfg(windows)]
pub use windows_impl::install;

#[cfg(windows)]
fn crash_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-timestamp".to_string())
}
