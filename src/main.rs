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

mod display;
mod event;

use clap_config_file::ClapConfigFile;
use anyhow::Result;
use x11rb::rust_connection::RustConnection;

#[derive(ClapConfigFile)]
#[config_file_name = "config"]
#[config_file_formats = "yaml,toml,json"]
struct Config {
    /// Record file type
    #[config_arg(short = 'd', name = "display", default_value = ":0", accept_from = "cli_only")]
    dpy_name: String,
}

#[derive(Default)]
struct Subtle {
    conn: Option<RustConnection>,
}

fn print_version() {
    println!(r#"
{} {} - Copyright (c) 2025-present {}
Released under the GNU Public License
Compiled for X11"#,
             env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
}

fn main() -> Result<()> {
    let (config, path, _format) = Config::parse_info();
    
    
    let mut subtle = Subtle::default();
    
    print_version();
    println!("Loaded config from: {:?}", path.unwrap_or_default());
    println!("Config: {:?}", config);
    
    display::init(&config, &mut subtle)?;
    
    display::configure(&config, &subtle)?;
    
    // Run event handler
    if let Err(e) = event::handle_requests(&mut subtle) {
        eprintln!("Error: {}", e);
    }
    
    display::finish(&mut subtle)?;
    
    println!("Exit");
    
    Ok(())
}
