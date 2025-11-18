///
/// @package subtle-rs
///
/// @file Tag functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use regex::{Regex, RegexBuilder};
use anyhow::Result;
use derive_builder::Builder;
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::client::Client;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;

bitflags! {
    /// Config and state-flags for [`Tags`]
    #[derive(Default, Debug, Clone)]
    pub(crate) struct TagFlags: u32 {
        const GRAVITY = 1 << 0; // Gravity property
        const GEOMETRY = 1 << 1; // Geometry property
        const POSITION = 1 << 2; // Position property
        const PROC = 1 << 3; // Tagging proc
    }
}

#[derive(Default, Builder)]
#[builder(default)]
pub(crate) struct Tag {
    pub(crate) flags: TagFlags,
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,

    pub(crate) screen_id: usize,
    pub(crate) gravity_id: usize,
    pub(crate) geom: Rectangle,
}

impl Tag {
    pub(crate) fn matches(&self, client: &Client) -> bool {
        if let Some(regex) = self.regex.as_ref() {
            return regex.is_match(&*client.name)
                || regex.is_match(&*client.instance)
                || regex.is_match(&*client.klass);
        }

        false
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(name={}, regex={:?})", self.name, self.regex)
    }
}

/// Check config and init all tag related options
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
    for tag_values in config.tags.iter() {
        let mut builder = TagBuilder::default();
        let mut flags = TagFlags::empty();

        if let Some(MixedConfigVal::S(value)) = tag_values.get("name") {
            builder.name(value.to_string());
        }

        if let Some(MixedConfigVal::S(value)) = tag_values.get("match") {
            builder.regex(Some(RegexBuilder::new(value)
                .case_insensitive(true)
                .build()?));
        }

        if let Some(MixedConfigVal::S(value)) = tag_values.get("gravity") {

            // Enable gravity only when gravity can be found
            if let Some(grav_id) = subtle.gravities.iter().position(|grav| grav.name.eq(value)) {
                flags.insert(TagFlags::GRAVITY);
                builder.gravity_id(grav_id);
            }
        }

        if let Some(MixedConfigVal::VI(value)) = tag_values.get("geometry") {
            if 4 == value.len() {
                flags.insert(TagFlags::GEOMETRY);
                builder.geom(Rectangle {
                    x: value[0] as i16,
                    y: value[1] as i16,
                    width: value[2] as u16,
                    height: value[3] as u16,
                });
            }
        }

        // Handle geometry
        if let Some(MixedConfigVal::VI(value)) = tag_values.get("position") {
            if flags.contains(TagFlags::GEOMETRY) {
                warn!("Tags cannot use both geometry and position");
            } else if 2 == value.len() {
                flags.insert(TagFlags::POSITION);
                builder.geom(Rectangle {
                    x: value[0] as i16,
                    y: value[1] as i16,
                    ..Rectangle::default()
                });
            }
        }

        builder.flags(flags);

        subtle.tags.push(builder.build()?);
    }
    
    // Sanity check
    if subtle.tags.is_empty() {
        let mut builder = TagBuilder::default();

        builder.name("default".into());

        subtle.tags.push(builder.build()?);
    }

    publish(subtle)?;
    
    debug!("{}", function_name!());
    
    Ok(())
}

/// Publish and export all relevant atoms to allow IPC
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    let mut tags: Vec<&str> = Vec::with_capacity(subtle.tags.len());

    for tag in subtle.tags.iter() {
        tags.push(&*tag.name);
    }

    conn.change_property8(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_TAG_LIST,
                          AtomEnum::STRING, tags.join("\0").as_bytes())?.check()?;

    conn.flush()?;

    debug!("{}: ntags={}", function_name!(), subtle.tags.len());
    
    Ok(())
}
