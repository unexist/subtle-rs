///
/// @package subtle-rs
///
/// @file Plugin functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
//

use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use extism::{host_fn, Manifest, UserData, Wasm, PTR};
use anyhow::{Context, Result};
use derive_builder::Builder;
use log::{debug, info};
use stdext::function_name;
use time::{format_description, OffsetDateTime};
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;

#[derive(Builder, Debug)]
#[builder(build_fn(skip))]
pub(crate) struct Plugin {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) interval: i32,

    #[builder(setter(skip))]
    pub(crate) plugin: Rc<RefCell<extism::Plugin>>,
}

impl PluginBuilder {

    host_fn!(get_formatted_time(_user_data: (); format: String) -> String {
        let dt = OffsetDateTime::now_local()?;

        let parsed_format = format_description::parse(&*format)?;

        Ok(dt.format(&parsed_format)?)
    });

    host_fn!(get_memory(_user_data: ()) -> String {
        let contents = std::fs::read_to_string("/proc/meminfo")?;

        let mem_available = contents.lines()
            .find(|line| line.starts_with("MemAvailable"))
            .and_then(|l| l.split(" ").nth(3))
            .context("Cannot read available memory")?;
        let mem_total = contents.lines()
            .find(|line| line.starts_with("MemTotal"))
            .and_then(|line| line.split(" ").nth(7))
            .context("Cannot read total memory")?;
        let mem_free = contents.lines()
            .find(|line| line.starts_with("MemFree"))
            .and_then(|line| line.split(" ").nth(8))
            .context("Cannot read free memory")?;

       Ok(format!("{} {} {}", mem_total, mem_available, mem_free))
    });

    host_fn!(get_battery(_user_data: (); battery_idx: String) -> String {
        let charge_full = std::fs::read_to_string(
            format!("/sys/class/power_supply/BAT{}/charge_full", battery_idx))?;
        let charge_now = std::fs::read_to_string(
            format!("/sys/class/power_supply/BAT{}/charge_now", battery_idx))?;

        Ok(format!("{} {}", charge_full, charge_now))
    });

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
    pub(crate) fn build(&self) -> Result<Plugin> {
        let url = self.url.clone().context("Url not set")?;

        // Load wasm plugin
        let wasm = Wasm::file(url.clone());
        let manifest = Manifest::new([wasm]);

        let plugin = extism::PluginBuilder::new(&manifest)
            .with_wasi(true)
            .with_function("get_formatted_time", [PTR], [PTR],
                           UserData::default(), Self::get_formatted_time)
            .with_function("get_memory", [PTR], [PTR],
                           UserData::default(), Self::get_memory)
            .build()?;

        debug!("{}", function_name!());

        Ok(Plugin {
            name: self.name.clone().context("Name not set")?,
            url,
            interval: self.interval.unwrap(),
            plugin: Rc::new(RefCell::new(plugin)),
        })
    }
}

impl Plugin {

    /// Call the run method of the plugin
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`String`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn update(&self) -> Result<String> {
       let res = self.plugin.borrow_mut().call("run", "")?;

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

        // Finally create plugin
        let plugin = builder.build()?;

        info!("Loaded plugin ({})", plugin.name);

        subtle.plugins.push(plugin);
    }

    debug!("{}", function_name!());

    Ok(())
}