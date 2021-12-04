use backtrace::Backtrace;
use chrono::Local;
use colored::{ColoredString, Colorize};
use log::{Level, Log, Metadata, Record};
use std::fs::File;
use std::io::{LineWriter, Write};

const LOG_LEVEL: log::Level = log::Level::Trace;

pub struct LogState {
    file: LineWriter<File>,
}

impl LogState {
    pub fn new(file: File) -> Self {
        Self {
            file: LineWriter::new(file),
        }
    }
}

struct Logger;

fn level_color(level: Level) -> ColoredString {
    let s = level.to_string();
    match level {
        Level::Error => s.red(),
        Level::Warn => s.yellow(),
        _ => s.normal(),
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LOG_LEVEL
    }

    fn log(&self, record: &Record) {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
        if !crate::test::has_test_data() {
            eprintln!(
                "{} [{}] [{}] [{}]: {}",
                now,
                level_color(record.level()),
                std::thread::current().name().unwrap_or("<unnamed thread>"),
                record.module_path().unwrap_or(""),
                record.args()
            );
        } else {
            crate::test::with_test_data(|td| {
                let mut log = td.log_state.lock();
                let mut path = record.module_path().unwrap_or("");
                if let Some(p) = path.strip_prefix("winit_it::") {
                    if record.metadata().level() == log::Level::Error {
                        td.error.set(true);
                    }
                    path = p;
                }
                writeln!(
                    &mut log.file,
                    "{} [{}] [{}]: {}",
                    now,
                    record.metadata().level(),
                    path,
                    record.args()
                )
                .unwrap();
            })
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(LOG_LEVEL.to_level_filter());
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };
        let bt = Backtrace::default();
        match info.location() {
            Some(location) => {
                log::error!(
                    "panic at '{}': {}:{}\n{:?}",
                    msg,
                    location.file(),
                    location.line(),
                    bt,
                );
            }
            None => log::error!(
                target: "panic",
                "panic at '{}'\n{:?}",
                msg,
                bt,
            ),
        }
    }));
}
