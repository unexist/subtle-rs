#![cfg_attr(debug_assertions, allow(dead_code, unused_variables, unused_assignments))]

///
/// @package subtle-rs
///
/// @file Main functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

#[cfg(test)]
mod tests;

/// Main module
mod subtle;
/// Display handling module
mod display;
/// Event handling module
mod event;
/// Client module
mod client;
/// View module
mod view;
/// Tag module
mod tag;
/// Screen module
mod screen;
/// Gravity module
mod gravity;
/// Log facility
mod logger;
/// Config module
mod config;
/// Grab module
mod grab;
/// EWMH module
mod ewmh;
/// Helper module to ease tagging
mod tagging;
/// Style module
mod style;
/// Font module
mod font;
/// Panel module
mod panel;
/// Helper module for spacing
mod spacing;
/// Icon module
mod icon;
/// Tray module
mod tray;
/// Plugin module
#[cfg(feature = "plugins")]
mod plugin;

use std::env;
use std::env::current_exe;
use std::sync::Arc;
use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use crate::config::Config;
use crate::font::Font;
use crate::style::StyleFlags;
use crate::subtle::{SubtleFlags, Subtle};

const DEFAULT_FONT_NAME: &str = "-*-*-*-*-*-*-14-*-*-*-*-*-*-*";

///  Install signal handler
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
fn install_signal_handler(subtle: &mut Subtle) -> Result<()> {
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&subtle.shutdown))
        .map_err(|e| anyhow!("Failed to register SIGINT handler: {}", e))?;
    
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&subtle.shutdown))
        .map_err(|e| anyhow!("Failed to register SIGTERM handler: {}", e))?;
    
    Ok(())
}

/// Print version info
fn print_version() {
    info!("{} {} - Copyright (c) 2025-present {}",
        env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
    info!("Released under the GNU GPLv3");
    info!("Compiled for X11");
}

/// Sanity-check configuration
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
fn sanity_check(subtle: &mut Subtle) -> Result<()> {

    // Check and update screens
    for (screen_idx, screen) in subtle.screens.iter_mut().enumerate() {
        screen.view_idx.set(if screen_idx < subtle.views.len() { screen_idx as isize } else { -1 });
    }

    // Enforce sane defaults
    if -1 == subtle.title_style.min_width {
        subtle.title_style.min_width = 50;
    }

    // Check fonts
    if !subtle.title_style.flags.intersects(StyleFlags::FONT) {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let font = Font::new(conn, DEFAULT_FONT_NAME)?;

        subtle.title_style.font_id = subtle.fonts.len() as isize;
        subtle.title_style.flags.insert(StyleFlags::FONT);

        subtle.fonts.push(font);
    }

    Ok(())
}

fn configure(config: &Config, subtle: &mut Subtle) -> Result<()> {
    display::init(&config, subtle)?;
    ewmh::init(&config, subtle)?;
    style::init(&config, subtle)?;
    #[cfg(feature = "plugins")]
    plugin::init(&config, subtle)?; // Must be before screen init
    screen::init(&config, subtle)?;
    gravity::init(&config, subtle)?;
    tag::init(&config, subtle)?;
    view::init(&config, subtle)?;
    grab::init(&config, subtle)?;

    sanity_check(subtle)?;

    Ok(())
}

fn run(subtle: &mut Subtle) -> Result<()> {
    // Prepare the stage
    style::update(subtle)?;
    screen::resize(subtle)?;

    display::claim(subtle)?;
    display::configure(&subtle)?;
    display::publish(&subtle)?;
    display::scan(subtle)?;

    // Run event handler
    event::event_loop(&subtle)?;

    Ok(())
}

/// Main function
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
fn main() -> Result<()> {
    // Load config
    let (config, path, _format) = Config::parse_info();

    logger::init(&config)?;

    info!("Reading file `{:?}'", path.unwrap_or_default());
    debug!("Config: {:?}", config);

    let mut subtle = Subtle::from(&config);

    install_signal_handler(&mut subtle)?;
    print_version();

    if let Err(err) = configure(&config, &mut subtle) {
        error!("Failed to configure: {:?}", err);
    } else {
        drop(config);

        if let Err(err) = run(&mut subtle) {
            error!("Failed to configure: {:?}", err);
        }
    }

    // Tidy up
    ewmh::finish(&subtle)?;
    display::finish(&mut subtle)?;

    // Restart if necessary
    if subtle.flags.contains(SubtleFlags::RESTART) {
        info!("Restarting");

        // When this actually returns something went wrong
        let err = exec::execvp(current_exe()?.as_os_str(), env::args());

        error!("Failed to restart: {:?}", err);
    }

    info!("Exit");
    
    Ok(())
}
