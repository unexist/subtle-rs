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
use x11rb::protocol::xproto::Rectangle;
use crate::config::Config;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const GRAVITY = 1 << 0; // Gravity property
        const GEOMETRY = 1 << 1; // Geometry property
        const POSITION = 1 << 2; // Position property
        const PROC = 1 << 3; // Tagging proc
    }
}

#[derive(Default)]
pub(crate) struct Tag {
    pub(crate) flags: Flags,
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,
    
    pub(crate) screen_id: usize,
    pub(crate) gravity_id: usize,
    pub(crate) geom: Rectangle,
}

impl Tag {
    pub(crate) fn new(name: &str, regex: &str) -> Result<Self> {
        let tag = Self {
            name: name.into(),
            regex: Some(Regex::new(regex)?),
            ..Default::default()
        };

        debug!("New: {}", tag);
        
        Ok(tag)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}", self.name)
    }
}

pub(crate) fn init(_config: &Config, _subtle: &mut Subtle) -> Result<()> {
    debug!("Init");
    
    Ok(())
}