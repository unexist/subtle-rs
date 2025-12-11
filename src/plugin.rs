///
/// @package subtle-rs
///
/// @file Plugin functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
//

use std::fmt;
use std::cell::OnceCell;
use extism::{Manifest, Wasm};
use anyhow::{anyhow, Context, Result};
use derive_builder::Builder;
use log::debug;
use stdext::function_name;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;

#[derive(Default, Builder, Debug)]
pub(crate) struct Plugin {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) interval: i32,

    #[builder(setter(skip))]
    pub(crate) plugin: OnceCell<extism::Plugin>,
}

impl Plugin {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin
    /// * `url` - Url to wasm file
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Plugin`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(name: String, url: String) -> Result<Self> {
        let plugin = Self {
            name: name.clone(),
            url: url.clone(),
            ..Default::default()
        };

        // Load wasm plugin
        let wasm_url = Wasm::url(url);
        let manifest = Manifest::new([wasm_url]);

        let wasm = extism::Plugin::new(&manifest, [], true)?;

        plugin.plugin.set(wasm).map_err(|e| anyhow!("Plugin already set?"))?;

        debug!("{}: plugin={}", function_name!(), plugin);

        Ok(plugin)
    }

    /// Call the run method of the plugin
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`String`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn update(&mut self) -> Result<String> {
       let res = self.plugin.get_mut()
           .context("Plugin not loaded")?.call("run", "")?;

        debug!("{}: res={}", function_name!(), res);

        Ok(res)
    }
}

impl fmt::Display for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "url={}, interval={}", self.url, self.interval)
    }
}

/// Check config and init all plugin related options
///
/// # Arguments
///
/// * `config` - Config values read either from args or config file
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    for values in config.plugins.iter() {
        let mut builder = PluginBuilder::default();

        if let Some(MixedConfigVal::S(value)) = values.get("name") {
            builder.name(value.to_string());
        }

        if let Some(MixedConfigVal::I(value)) = values.get("interval") {
            builder.interval(*value);
        }

        if let Some(MixedConfigVal::S(value)) = values.get("url") {
            builder.url(value.to_string());
        }

        subtle.plugins.push(builder.build()?);
    }

    debug!("{}", function_name!());

    Ok(())
}