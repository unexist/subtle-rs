///
/// @package subtle-rs
///
/// @file Main file
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use clap_config_file::ClapConfigFile;

#[derive(ClapConfigFile)]
#[config_file_name = "config"]
#[config_file_formats = "yaml,toml,json"]
struct Config {
    /// Record file type
    #[config_arg(default_value = "adoc")]
    file_type: String,
}

fn main() {
    let (config, path, _format) = Config::parse_info();
    
    println!("Loaded config from: {:?}", path.unwrap_or_default());
    println!("Config: {:?}", config);
    
    // Run actual command
    if let Err(e) = handle_command(&config) {
        eprintln!("Error: {}", e);
    }
}
