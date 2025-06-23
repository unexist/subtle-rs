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

use clap_config_file::ClapConfigFile;

#[derive(ClapConfigFile)]
#[config_file_name = "subtle"]
#[config_file_formats = "yaml,toml,json"]
pub(crate) struct Config {
    /// Connect to DISPLAY
    #[config_arg(short = 'd', name = "display", default_value = ":0", accept_from = "cli_only")]
    pub(crate) display: String,

    /// Set logging level LEVEL
    #[config_arg(short = 'l', name = "level", default_value = "", accept_from = "cli_only")]
    pub(crate) loglevel: String,

    /// Print debugging messages
    #[config_arg(short = 'D', name = "debug", default_value = false, accept_from = "cli_only")]
    pub(crate) debug: bool,

    #[config_arg(name = "gravity", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) gravities: Vec<Vec<i64>>,
}
