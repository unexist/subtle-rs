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
use regex::{Regex, RegexBuilder};
use anyhow::Result;
use derive_builder::Builder;
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, PropMode};
use x11rb::wrapper::ConnectionExt;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::Subtle;
use crate::tagging::Tagging;

bitflags! {
    #[derive(Default, Debug, Clone)]
    pub(crate) struct ViewFlags: u32 {
        const MODE_ICON = 1 << 0; // View icon
        const MODE_ICON_ONLY = 1 << 1; // Icon only
        const MODE_DYNAMIC = 1 << 2; // Dynamic views
        const MODE_STICK = 1 << 3; // Stick view
    }
}

#[derive(Default, Builder)]
#[builder(default)]
pub(crate) struct View {
    pub(crate) flags: ViewFlags,
    pub(crate) tags: Tagging,
    
    pub(crate) name: String,
    pub(crate) regex: Option<Regex>,

    pub(crate) text_width: u16,
}

impl View {
    fn retag(&mut self, subtle: &Subtle) {
        for (tag_idx, tag) in subtle.tags.iter().enumerate() {
            if let Some(regex) = self.regex.as_ref()
                && regex.is_match(&*tag.name)
            {
                self.tags = Tagging::from_bits_retain(1 << tag_idx);
            }
        }

        debug!("{}: {}", function_name!(), self);
    }

    pub(crate) fn focus(&self, subtle: &Subtle) -> Result<()> {
        debug!("{}: {}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(name={}, regex={:?}, tags={:?})", self.name, self.regex, self.tags)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    for values in config.views.iter() {
        let mut builder = ViewBuilder::default();

        if let Some(MixedConfigVal::S(name)) = values.get("name") {
            builder.name(name.into());
        }

        if let Some(MixedConfigVal::S(value)) = values.get("match") {
            builder.regex(Some(RegexBuilder::new(value)
                .case_insensitive(true)
                .build()?));
        }

        // Apply tagging
        let mut view = builder.build()?;

        view.retag(subtle);

        subtle.views.push(view)
    }

    // Sanity check
    if subtle.views.is_empty() {
        let mut builder = ViewBuilder::default();

        builder.name("default".into());

        subtle.views.push(builder.build()?);
    }

    publish(subtle)?;

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    let mut names: Vec<&str> = Vec::with_capacity(subtle.views.len());
    let mut tags: Vec<u32> = Vec::with_capacity(subtle.views.len());
    let mut icons: Vec<u32> = Vec::with_capacity(subtle.views.len());

    for view in subtle.views.iter() {
        names.push(&*view.name);
        tags.push(view.tags.bits());
        icons.push(0);
    }

    // EWMH: Tags
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_VIEW_TAGS,
                           AtomEnum::CARDINAL, &tags)?.check()?;

    // EWMH: Icons
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_VIEW_ICONS,
                           AtomEnum::CARDINAL, &icons)?.check()?;

    // EWMH: Desktops
    let data: [u32; 1] = [subtle.views.len() as u32];

    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_NUMBER_OF_DESKTOPS,
                           AtomEnum::CARDINAL, &data)?.check()?;

    conn.change_property8(PropMode::REPLACE, default_screen.root, atoms._NET_DESKTOP_NAMES,
                          AtomEnum::STRING, names.join("\0").as_bytes())?.check()?;
    
    // EWMH: Current desktop
    let data: [u32; 1] = [0];
    
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_CURRENT_DESKTOP,
                           AtomEnum::CARDINAL, &data)?.check()?;
    
    conn.flush()?;

    debug!("{}: nviews={}", function_name!(), subtle.views.len());

    Ok(())
}
