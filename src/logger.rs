///
/// @package subtle-rs
///
/// @file Logger functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use log::LevelFilter;
use crate::Config;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LogLevel {
    None,
    Info,
    Warnings,
    Error,
    Deprecated,
    Events,
    XError,
    Subtle,
    Debug
}

impl From<&String> for LogLevel {
    fn from(level: &String) -> Self {
        match level.to_lowercase().as_str() {
            "none" => LogLevel::None,
            "info" => LogLevel::Info,
            "warnings" => LogLevel::Warnings,
            "errors" => LogLevel::Error,
            "deprecated" => LogLevel::Deprecated,
            "events" => LogLevel::Events,
            "xerror" => LogLevel::XError,
            "subtle" => LogLevel::Subtle,
            "debug" => LogLevel::Debug,
            _ => LogLevel::Info,
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::None => LevelFilter::Off,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warnings => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Debug => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

pub(crate) fn init(config: &Config) -> anyhow::Result<()> {
    let mut level = LogLevel::from(&config.loglevel);
    
    if config.debug {
        level = LogLevel::Debug;
    }

    let filter = LevelFilter::from(level);

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_level(filter)
        .try_init()?;

    Ok(())
}