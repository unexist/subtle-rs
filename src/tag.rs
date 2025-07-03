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
use crate::config::{Config, Mixed};
use crate::gravity::Gravity;
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
    pub(crate) fn new(name: &str) -> Self {
        let tag = Self {
            name: name.into(),
            ..Default::default()
        };

        debug!("New: {}", tag);
        
        tag
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, regex={:?}", self.name, self.regex)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    for (name, values) in config.tags.iter() {
        let mut tag = Tag::new(name);

        if values.contains_key("match") {
            if let Mixed::S(value) = values.get("match").unwrap() {
                tag.regex = Some(Regex::new(value)?);
            }
        }
        
        subtle.tags.push(tag)
    }
    
    debug!("Init");
    
    Ok(())
}