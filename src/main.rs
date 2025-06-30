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

#[cfg(test)]
mod tests;

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
mod config;
mod grab;
mod ewmh;

use std::sync::atomic;
use anyhow::{Context, Result};
use log::{debug, info};
use crate::config::Config;
use crate::subtle::Subtle;

fn install_signal_handler(subtle: &mut Subtle) -> Result<()> {
    let running = subtle.running.clone();

    ctrlc::set_handler(move || {
        running.store(false, atomic::Ordering::SeqCst);
    }).with_context(|| "Failed to set CTRL-C handler")
}

fn print_version() {
    info!("{} {} - Copyright (c) 2025-present {}",
        env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
    info!("Released under the GNU GPLv3");
    info!("Compiled for X11");
}

fn main() -> Result<()> {
    // Load config
    let (config, path, _format) = Config::parse_info();

    logger::init(&config)?;

    info!("Reading file `{:?}'", path.unwrap_or_default());
    debug!("Config: {:?}", config);

    // Init subtle
    let mut subtle = Subtle::from(&config);

    install_signal_handler(&mut subtle)?;
    print_version();

    display::init(&config, &mut subtle)?;
    ewmh::init(&config, &mut subtle)?;
    gravity::init(&config, &mut subtle)?;
    
    drop(config);

    display::claim(&mut subtle)?;
    display::configure(&subtle)?;
    display::scan(&mut subtle)?;

    // Run event handler
    event::handle_requests(&mut subtle)?;
    
    display::finish(&mut subtle)?;
    
    info!("Exit");
    
    Ok(())
}

