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
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use extism::{host_fn, Manifest, UserData, Wasm, PTR};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use derive_builder::Builder;
use extism::ValType::I32;
use log::{debug, info};
use stdext::function_name;
use itertools::Itertools;
use regex::Regex;
use lazy_static::lazy_static;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;

#[derive(Debug)]
pub(crate) struct Plugin {
    /// Name of the plugin
    pub(crate) name: String,
    /// Update interval
    pub(crate) interval: i32,
    /// Extism plugin
    pub(crate) plugin: Rc<RefCell<extism::Plugin>>,
}

#[derive(Builder)]
#[builder(name = "PluginBuilder", build_fn(skip))]
pub(crate) struct PluginBuilderSeed {
    /// Name of the plugin
    pub(crate) name: String,
    /// Path or file url to wasm file
    url: String,
    /// Update interval
    pub(crate) interval: i32,
    /// Plugin config
    pub(crate) config: HashMap<String, String>,
}

/// Lazy global for all instances of this plugin
type CpuUserData = Vec<(i32, i32, i32)>;

lazy_static! {
    static ref CPU_USER_DATA: UserData<CpuUserData> = UserData::new(CpuUserData::new());
}

host_fn!(get_formatted_time(_user_data: (); format: String) -> String {
    let current_local: DateTime<Local> = Local::now();

    Ok(current_local.format(&*format).to_string())
});

host_fn!(get_memory(_user_data: ()) -> String {
    let (mem_available, mem_total, mem_free) = std::fs::read_to_string("/proc/meminfo")?
        .lines()
        .filter(|line| line.starts_with("MemAvailable") || line.starts_with("MemTotal") || line.starts_with("MemFree"))
        .map(|line| line.split_whitespace().nth(1).and_then(|v| v.parse::<i32>().ok()))
        .collect_tuple()
        .context("Cannot read `/proc/meminfo`")?;

   Ok(format!("{} {} {}", mem_total.unwrap_or(1), mem_available.unwrap_or(0), mem_free.unwrap_or(0)))
});

host_fn!(get_battery(_user_data: (); battery_slot: String) -> String {
    let charge_full = std::fs::read_to_string(
        format!("/sys/class/power_supply/BAT{}/charge_full", battery_slot))?;
    let charge_now = std::fs::read_to_string(
        format!("/sys/class/power_supply/BAT{}/charge_now", battery_slot))?;

    Ok(format!("{} {}", charge_full.trim(), charge_now.trim()))
});

host_fn!(get_cpu(user_data: CpuUserData;) -> bool {
    let plug_data = user_data.get()?;
    let mut plug_data = plug_data.lock().unwrap();

    plug_data.clear();

    let regex = Regex::new(r"cpu(\d+) (\d+) (\d+) (\d+)")?;

    for line in std::fs::read_to_string("/proc/stat")?.lines() {
        if let Some(cap) = regex.captures(line) {
            let cpu_user = cap.get(1).map_or(0, |v| v.as_str().parse::<i32>().unwrap_or(0));
            let cpu_nice = cap.get(2).map_or(0, |v| v.as_str().parse::<i32>().unwrap_or(0));
            let cpu_system = cap.get(3).map_or(0, |v| v.as_str().parse::<i32>().unwrap_or(0));

            plug_data.push((cpu_user, cpu_nice, cpu_system));
        }
    }

   Ok(true)
});

impl PluginBuilder {

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
    pub(crate) fn build(&mut self) -> Result<Plugin> {
        let url = self.url.clone().context("Url not set")?;

        let config = self.config.take().unwrap_or_default();

        // Load wasm plugin
        let wasm = Wasm::file(url);
        let manifest = Manifest::new([wasm])
            .with_timeout(Duration::from_secs(5))
            .with_config(config.into_iter());

        let plugin = extism::PluginBuilder::new(&manifest)
            .with_wasi(true)
            .with_function("get_formatted_time", [PTR], [PTR],
                           UserData::default(), get_formatted_time)
            .with_function("get_memory", [PTR], [PTR],
                           UserData::default(), get_memory)
            .with_function("get_battery", [PTR], [PTR],
                           UserData::default(), get_battery)
            .with_function("get_cpu", [PTR], [I32],
                           CPU_USER_DATA.clone(), get_cpu)
            .build()?;

        debug!("{}", function_name!());

        Ok(Plugin {
            name: self.name.clone().context("Name not set")?,
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
        write!(f, "name={}, interval={}", self.name, self.interval)
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

        if let Some(MixedConfigVal::S(value)) = values.get("url") {
            builder.url(value.to_string());
        }

        if let Some(MixedConfigVal::I(value)) = values.get("interval") {
            builder.interval(*value);
        }

        if let Some(MixedConfigVal::MSS(values)) = values.get("config") {
            let config: HashMap<String, String> = values.into_iter()
                .map(|entry| (String::from(entry.0), String::from(entry.1)))
                .collect();

            builder.config(config);
        }

        // Finally create actual plugin
        let plugin = builder.build()?;

        info!("Loaded plugin ({})", plugin.name);

        subtle.plugins.push(plugin);
    }

    debug!("{}", function_name!());

    Ok(())
}