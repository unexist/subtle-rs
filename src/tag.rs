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
use crate::rect::Rect;

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
    
    pub(crate) screen_id: u32,
    pub(crate) gravity_id: u32,
    pub(crate) geom: Rect,
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