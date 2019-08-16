extern crate log;

mod error;
mod flamegraph;
mod symbols;
mod trace;

use log::{Level, Log, Metadata, Record};
use std::collections::BTreeMap;
use std::env;
use std::io::{self, BufRead};
use std::process::exit;
use std::str::FromStr;

use error::Error;
use symbols::Symbol;

struct Logger {
    level: Level,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level_string = record.level().to_string();
            let target = if record.target().len() > 0 {
                record.target()
            }
            else {
                record.module_path().unwrap_or_default()
            };

            eprintln!("{:<5} [{}] {}", level_string, target, record.args());
        }
    }

    fn flush(&self) {
    }
}

enum Mode {
    Trace,
    FlameGraph,
}

#[derive(Eq, PartialEq)]
pub enum ISA {
    X86_64,
    ARM,
}

pub fn with_stdin_lines<F>(syms: &BTreeMap<usize, Symbol>, mut func: F) -> Result<(), Error>
where
    F: FnMut(&BTreeMap<usize, Symbol>, &mut io::StdoutLock, &str) -> Result<(), Error>,
{
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        func(syms, &mut writer, &line)?;
        line.clear();
    }

    Ok(())
}

fn usage(prog: &str) -> ! {
    eprintln!(
        "Usage: {} (x86_64|arm) (trace|flamegraph) [<binary>...]",
        prog
    );
    exit(1)
}

fn main() -> Result<(), error::Error> {
    let level = Level::from_str(&env::var("RUST_LOG").unwrap_or("error".to_string()))?;
    log::set_boxed_logger(Box::new(Logger { level }))?;
    log::set_max_level(level.to_level_filter());

    let args: Vec<String> = env::args().collect();

    let isa = match args.get(1) {
        Some(isa) if isa == "x86_64" => ISA::X86_64,
        Some(isa) if isa == "arm" => ISA::ARM,
        _ => usage(&args[0]),
    };

    let mode = match args.get(2) {
        Some(mode) if mode == "trace" => Mode::Trace,
        Some(mode) if mode == "flamegraph" => Mode::FlameGraph,
        _ => usage(&args[0]),
    };

    let mut syms = BTreeMap::new();
    for f in &args[3..] {
        symbols::parse_symbols(&mut syms, f)?;
    }

    match mode {
        Mode::Trace => trace::generate(&syms),
        Mode::FlameGraph => flamegraph::generate(&isa, &syms),
    }
}
