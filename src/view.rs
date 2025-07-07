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
use stdext::function_name;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;
use crate::tag::Tag;
use crate::tagging::Tagging;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const MODE_ICON = 1 << 0; // View icon
        const MODE_ICON_ONLY = 1 << 1; // Icon only
        const MODE_DYNAMIC = 1 << 2; // Dynamic views
        const MODE_STICK = 1 << 3; // Stick view
    }
}

#[derive(Default)]
pub(crate) struct View {
    pub(crate) flags: Flags,
    pub(crate) tags: Tagging,
    
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,
}

impl View {
    pub(crate) fn new(name: &str) -> Self {
        let view = Self {
            name: name.into(),
            ..Default::default()
        };

        debug!("{}: {}", function_name!(), view);

        view
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}", self.name)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    for (name, values) in config.views.iter() {
        let mut view = View::new(name);

        if values.contains_key("match") {
            if let Some(MixedConfigVal::S(value)) = values.get("match") {
                view.regex = Some(Regex::new(value)?);
            }
        }

        subtle.views.push(view)
    }

    debug!("{}", function_name!());

    Ok(())
}
