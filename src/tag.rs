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
use anyhow::{anyhow, Result};
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::client::Client;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct TagFlags: u32 {
        const GRAVITY = 1 << 0; // Gravity property
        const GEOMETRY = 1 << 1; // Geometry property
        const POSITION = 1 << 2; // Position property
        const PROC = 1 << 3; // Tagging proc
    }
}

#[derive(Default)]
pub(crate) struct Tag {
    pub(crate) flags: TagFlags,
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,
    
    pub(crate) screen_id: usize,
    pub(crate) gravity_id: usize,
    pub(crate) geom: Rectangle,
}

impl Tag {
    pub(crate) fn new(name: &str) -> Result<Self> {
        if name.is_empty() {
            return Err(anyhow!("Empty tag name"))
        }

        let tag = Self {
            name: name.into(),
            ..Default::default()
        };

        debug!("{}: {}", function_name!(), tag);
        
        Ok(tag)
    }
    
    pub(crate) fn matches(&self, client: &Client) -> bool {
        true
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, regex={:?}", self.name, self.regex)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    for (name, values) in config.tags.iter() {
        let mut tag = Tag::new(name)?;

        // Handle match
        if let Some(MixedConfigVal::S(value)) = values.get("match") {
            tag.regex = Some(Regex::new(value)?);
        }

        subtle.tags.push(tag)
    }
    
    // Sanity check
    if subtle.tags.is_empty() {
        let tag = Tag::new("default")?;
        
        subtle.tags.push(tag);
    }
    
    publish(subtle)?;
    
    debug!("{}", function_name!());
    
    Ok(())
}

pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];

    let mut tags: Vec<&str> = Vec::with_capacity(subtle.tags.len());

    for tag in subtle.tags.iter() {
        tags.push(&*tag.name);
    }

    conn.change_property8(PropMode::REPLACE, screen.root, atoms.SUBTLE_TAG_LIST,
                          AtomEnum::STRING, tags.join("\0").as_bytes())?.check()?;

    conn.flush()?;

    debug!("{}: tags={}", function_name!(), subtle.tags.len());
    
    Ok(())
}
