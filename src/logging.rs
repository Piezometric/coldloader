use {std::{ffi::CString, str::FromStr as _}, winapi::um::winuser::MessageBoxA};
use std::{panic, sync::Once};
use log::LevelFilter;
use log4rs::{append::file::FileAppender, config::{Appender, Root}, encode::pattern::PatternEncoder, Config as Log4rsConfig};

static LOGGER: Once = Once::new();

pub fn init_logger(is_proxy: bool) {
    LOGGER.call_once(|| {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let prefix = if is_proxy { "coldloader_proxy" } else { "coldloader" };
        let log_file_path = format!("{}_{}.log", prefix, timestamp);

        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%dT%H:%M:%S%.3f)}] [{l}]: {m}{n}")))
            .build(log_file_path)
            .unwrap();

        let config = Log4rsConfig::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(
                Root::builder()
                    .appender("logfile")
                    .build(LevelFilter::Info),
            )
            .unwrap();

        let _handle = log4rs::init_config(config).unwrap();

        let mode = if is_proxy { "PROXY" } else { "STANDALONE" };
        log::info!("Logger initialized in {} mode", mode);
    });
}

pub fn setup_panic_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else {
            "Unknown panic message".to_string()
        };

        let location = if let Some(location) = panic_info.location() {
            format!("{}:{}", location.file(), location.line())
        } else {
            "unknown location".to_string()
        };

        unsafe {
            MessageBoxA(
                std::ptr::null_mut(),
                format!("Panic occurred at {}: {}\0", location, message).as_ptr() as *const i8,
                "Panic\0".as_ptr() as *const i8,
                0,
            );
        }

        log::error!("Panic occurred at {}: {}", location, message);
    }));
}

pub fn message_box(message: &str) {
    let message = CString::from_str(message).unwrap();

    unsafe {
        MessageBoxA(
            std::ptr::null_mut(),
            message.as_ptr() as *const i8,
            "ColdLoader\0".as_ptr() as *const i8,
            0,
        );
    }
}
