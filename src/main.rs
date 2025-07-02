#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

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
mod gravity;
mod logger;
mod config;
mod grab;
mod ewmh;
mod tagging;

use std::env;
use std::env::current_exe;
use std::sync::{atomic, Arc};
use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use crate::config::Config;
use crate::subtle::{Flags, Subtle};

fn install_signal_handler(subtle: &mut Subtle) -> Result<()> {
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&subtle.exterminate))
        .map_err(|e| anyhow!("Failed to register SIGINT handler: {}", e))?;
    
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&subtle.exterminate))
        .map_err(|e| anyhow!("Failed to register SIGTERM handler: {}", e))?;
    
    Ok(())
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
    screen::init(&config, &mut subtle)?;
    gravity::init(&config, &mut subtle)?;
    tag::init(&config, &mut subtle)?;
    view::init(&config, &mut subtle)?;

    drop(config);

    display::claim(&mut subtle)?;
    display::configure(&subtle)?;
    display::scan(&mut subtle)?;

    // Run event handler
    event::event_loop(&mut subtle)?;
    
    display::finish(&mut subtle)?;
    
    // Restart if necessary
    if subtle.flags.contains(Flags::RESTART) {
        info!("Restarting");

        // When this actually returns something went wrong
        let err = exec::execvp(current_exe()?.as_os_str(), env::args());
        
        error!("Error: {:?}", err);
    }
    
    info!("Exit");
    
    Ok(())
}

