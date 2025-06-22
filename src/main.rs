///
/// @package subtle-rs
///
/// @file Main functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

mod subtle;
mod display;
mod event;
mod client;
mod view;
mod tag;
mod screen;
mod rect;
mod gravity;
mod logger;

use std::sync::atomic;
use clap_config_file::ClapConfigFile;
use anyhow::{Context, Result};
use log::{error, info, LevelFilter};
use crate::subtle::Subtle;

#[derive(ClapConfigFile)]
#[config_file_name = "subtle"]
#[config_file_formats = "yaml,toml,json"]
struct Config {
    /// Connect to DISPLAY
    #[config_arg(short = 'd', name = "display", default_value = ":0", accept_from = "cli_only")]
    display: String,

    /// Set logging level LEVEL
    #[config_arg(short = 'l', name = "level", default_value = "", accept_from = "cli_only")]
    loglevel: String,

    /// Print debugging messages
    #[config_arg(short = 'D', name = "debug", default_value = false, accept_from = "cli_only")]
    debug: bool,

    #[config_arg(name = "gravity", multi_value_behavior = "extend", accept_from = "config_only")]
    pub gravities: Vec<String>,
}

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

fn install_signal_handler(subtle: &mut Subtle) -> Result<()> {
    let running = subtle.running.clone();

    ctrlc::set_handler(move || {
        running.store(false, atomic::Ordering::SeqCst);
    }).with_context(|| "Failed to set CTRL-C handler")
}

fn print_version() {
    info!(r#"
{} {} - Copyright (c) 2025-present {}
Released under the GNU Public License
Compiled for X11"#,
             env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
}

fn main() -> Result<()> {
    let mut subtle = Subtle::default();

    // Load config
    let (config, path, _format) = Config::parse_info();

    logger::init(&config)?;

    info!("Reading file `{:?}'", path.unwrap_or_default());
    info!("Config: {:?}", config);

    install_signal_handler(&mut subtle)?;
    print_version();

    display::init(&config, &mut subtle)?;

    gravity::configure(&config, &mut subtle)?;
    display::configure(&config, &subtle)?;
    
    // Run event handler
    if let Err(e) = event::handle_requests(&mut subtle) {
        error!("Error: {}", e);
    }
    
    display::finish(&mut subtle)?;
    
    info!("Exit");
    
    Ok(())
}

