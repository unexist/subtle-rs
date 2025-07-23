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
use std::collections::HashMap;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum MixedConfigVal {
    S(String),
    VI(Vec<i32>),
    VS(Vec<String>),
    I(i32),
    B(bool),
}

#[derive(ClapConfigFile)]
#[config_file_name = "subtle"]
#[config_file_formats = "yaml,toml,json"]
pub(crate) struct Config {
    /// Connect to DISPLAY
    #[config_arg(short = 'd', default_value = ":0", accept_from = "cli_only")]
    pub(crate) display: String,
    
    /// Replace current window manager
    #[config_arg(short = 'r', default_value = false, accept_from = "cli_only")]
    pub(crate) replace: bool,

    /// Set logging level LEVEL
    #[config_arg(short = 'l', name = "level", default_value = "", accept_from = "cli_only")]
    pub(crate) loglevel: String,

    /// Print debugging messages
    #[config_arg(short = 'D', default_value = false, accept_from = "cli_only")]
    pub(crate) debug: bool,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) subtle: HashMap<String, MixedConfigVal>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) styles: HashMap<String, HashMap<String, MixedConfigVal>>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) gravities: HashMap<String, Vec<u16>>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) grabs: HashMap<String, String>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) tags: HashMap<String, HashMap<String, MixedConfigVal>>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) views: IndexMap<String, HashMap<String, MixedConfigVal>>,
}
