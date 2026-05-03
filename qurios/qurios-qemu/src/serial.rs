use core::fmt::Write;
use log_impl::LOG_LEVEL;
use spin::Mutex;
use uart_16550::{Config, Uart16550Tty};

#[cfg(target_arch = "x86_64")]
/// Initializes serial port and logger. sprint! and log macros after this.
pub fn init() {
    // SAFETY: Serial port address base is correct.
    SERIAL.lock().replace(SerialPort::new());
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LOG_LEVEL))
        .expect("Couldn't set the serial logger");
    log::info!("Logging initialized");
}
/// Initializes serial port and logger. sprint! and log macros after this.
pub fn init_with(port: SerialPort) {
    // SAFETY: Serial port address base is correct.
    SERIAL.lock().replace(port);
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LOG_LEVEL))
        .expect("Couldn't set the serial logger");
    log::info!("Logging initialized");
}

static SERIAL: spin::Mutex<Option<SerialPort>> = Mutex::new(None);

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL
        .lock()
        .as_mut()
        .unwrap()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

#[cfg(target_arch = "x86_64")]
struct SerialPort(Uart16550Tty<uart_16550::backend::PioBackend>);
#[cfg(target_arch = "riscv64")]
pub struct SerialPort(Uart16550Tty<uart_16550::backend::MmioBackend>);

impl SerialPort {
    pub fn new(#[cfg(target_arch = "riscv64")] pmo: usize) -> Self {
        let serial_port = cfg_select! {
            target_arch = "x86_64" => {
                unsafe { Uart16550Tty::new_port(0x3F8, Config::default()).unwrap() }
            }
            target_arch = "riscv64" => {
                unsafe {
                    use core::ptr::{self, NonNull};
                    let mmio_address = ptr::with_exposed_provenance_mut::<u8>(pmo + 0x1000_0000);
                    let mmio_address = NonNull::new(mmio_address).unwrap();
                    Uart16550Tty::new_mmio(mmio_address, 1, Config::default()).unwrap()
                }
            }
        };
        Self(serial_port)
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_str(s)
    }
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! sprint {
    ($($arg:tt)*) => {
        $crate::serial::_print(::core::format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! sprintln {
    () => ($crate::sprint!("\n"));
    ($fmt:expr) => ($crate::sprint!(::core::concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::sprint!(
        ::core::concat!($fmt, "\n"), $($arg)*));
}

/// Prints a debug expression to the serial port.
#[macro_export]
macro_rules! sdbg {
    () => {
        $crate::sprintln!("[{}:{}]", ::core::file!(), ::core::line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::sprintln!("[{}:{}] {} = {:#?}",
                    ::core::file!(), ::core::line!(), ::core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::sdbg!($val)),+,)
    };
}

struct Logger;
/// The global logger.
static LOGGER: Logger = Logger {};

mod log_impl {
    use log::{LevelFilter, Metadata, Record};

    use super::Logger;

    const fn log_level() -> LevelFilter {
        let level = core::option_env!("RUST_LOG");

        match level {
            Some("trace") => LevelFilter::Trace,
            Some("debug") => LevelFilter::Debug,
            Some("info") | None => LevelFilter::Info,
            Some("warn") => LevelFilter::Warn,
            Some("error") => LevelFilter::Error,
            Some(_) => core::panic!("Unknown log level in RUST_LOG env"),
        }
    }

    pub const LOG_LEVEL: LevelFilter = log_level();

    impl log::Log for Logger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= LOG_LEVEL
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                crate::sprint!("[{}]\t", record.level());
                if let (Some(file), Some(line)) = (record.file(), record.line()) {
                    crate::sprint!("@ {}:{}", file, line);
                }
                crate::sprintln!(" : {}", record.args());
            }
        }

        fn flush(&self) {}
    }
}
