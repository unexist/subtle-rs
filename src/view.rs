///
/// @package subtle-rs
///
/// @file View functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use regex::Regex;
use anyhow::Result;
use log::debug;
use crate::config::Config;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const MODE_ICON = 1 << 10; // View icon
        const MODE_ICON_ONLY = 1 << 11; // Icon only
        const MODE_DYNAMIC = 1 << 12; // Dynamic views
        const MODE_STICK = 1 << 13; // Stick view
    }
}

#[derive(Default)]
pub(crate) struct View {
    pub(crate) flags: Flags,
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,
}

impl View {
    pub(crate) fn new(name: &str, regex: &str) -> Result<Self> {
        let view = Self {
            name: name.into(),
            regex: Some(Regex::new(regex)?),
            ..Default::default()
        };

        debug!("New: {}", view);

        Ok(view)
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}", self.name)
    }
}

pub(crate) fn init(_config: &Config, _subtle: &mut Subtle) -> Result<()> {
    debug!("Init");

    Ok(())
}
