use ferros::debug_println;
use log::{Metadata, Record};

pub struct DebugLogger;

impl log::Log for DebugLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            debug_println!("{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
