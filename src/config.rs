///
/// @package subtle-rs
///
/// @file Config functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use clap_config_file::ClapConfigFile;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum MixedConfigVal {
    S(String),
    VI(Vec<i32>),
    VVI(Vec<Vec<i32>>),
    VS(Vec<String>),
    MVS(HashMap<String, Vec<String>>),
    MSS(HashMap<String, MixedConfigVal>),
    I(i32),
    B(bool),
}

impl From<&MixedConfigVal> for String {
    fn from(value: &MixedConfigVal) -> Self {
        match value {
            MixedConfigVal::S(value) => String::from(value),
            MixedConfigVal::I(value) => value.to_string(),
            MixedConfigVal::B(value) => value.to_string(),
            _ => todo!(),
        }
    }
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

    #[config_arg(name = "style", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) styles: Vec<HashMap<String, MixedConfigVal>>,

    #[config_arg(name = "gravity", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) gravities: Vec<HashMap<String, MixedConfigVal>>,

    #[config_arg(multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) grabs: HashMap<String, MixedConfigVal>,

    #[config_arg(name = "tag", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) tags: Vec<HashMap<String, MixedConfigVal>>,

    #[config_arg(name = "view", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) views: Vec<HashMap<String, MixedConfigVal>>,

    #[config_arg(name = "plugin", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) plugins: Vec<HashMap<String, MixedConfigVal>>,

    #[config_arg(name = "screen", multi_value_behavior = "extend", accept_from = "config_only")]
    pub(crate) screens: Vec<HashMap<String, MixedConfigVal>>,
}
