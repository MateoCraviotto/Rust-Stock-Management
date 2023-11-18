use clap::{builder::PossibleValue, ValueEnum};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Verbosity {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl std::fmt::Display for Verbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl std::str::FromStr for Verbosity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}

impl ValueEnum for Verbosity {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Verbosity::Debug,
            Verbosity::Info,
            Verbosity::Warn,
            Verbosity::Error,
            Verbosity::Fatal,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Verbosity::Debug => PossibleValue::new("debug").help("Log everything"),
            Verbosity::Info => PossibleValue::new("info")
                .help("Log whenever something important happens and onwards."),
            Verbosity::Warn => PossibleValue::new("warn")
                .help("Log whenever some runtime error, easily recoverable happens and onwards."),
            Verbosity::Error => PossibleValue::new("error")
                .help("Log whenever something fails, but it can be recovered and onwards"),
            Verbosity::Fatal => {
                PossibleValue::new("fatal").help("Log whenever something irrecoverable fails")
            }
        })
    }
}

#[macro_export]
macro_rules! debug {
    ($text: expr) => {{
        if crate::common::log::Verbosity::Debug as u8
            >= crate::VERBOSITY.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!("DEBUG: {}", $text)
        }
    }};
}

#[macro_export]
macro_rules! info {
    ($text: expr) => {{
        if crate::common::log::Verbosity::Info as u8
            >= crate::VERBOSITY.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!("INFO: {}", $text)
        }
    }};
}

#[macro_export]
macro_rules! warn {
    ($text: expr) => {{
        if crate::common::log::Verbosity::Warn as u8
            >= crate::VERBOSITY.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!("WARN: {}", $text)
        }
    }};
}

#[macro_export]
macro_rules! error {
    ($text: expr) => {{
        if crate::common::log::Verbosity::Error as u8
            >= crate::VERBOSITY.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!("ERROR: {}", $text)
        }
    }};
}

#[macro_export]
macro_rules! fatal {
    ($text: expr) => {{
        if crate::common::log::Verbosity::Fatal as u8
            >= crate::VERBOSITY.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!("FATAL: {}", $text)
        }
    }};
}

#[macro_export]
macro_rules! log_level {
    ($level: expr) => {
        tp::VERBOSITY.store($level as u8, std::sync::atomic::Ordering::Relaxed)
    };
}
