///
/// @package subtle-rs
///
/// @file Gravity functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use easy_min_max::{min, max, clamp};
use anyhow::{anyhow, Result};
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt;
use crate::Config;
use crate::config::MixedConfigVal;
use crate::subtle::Subtle;

bitflags! {
    /// Config and state-flags for [`Gravity`]
    #[derive(Default, Debug)]
    pub(crate) struct GravityFlags: u32 {
        /// Gravity tile gravity horizontally
        const HORZ = 1 << 0;
        /// Gravity tile gravity vertically
        const VERT = 1 << 1;
    }
}

#[derive(Default)]
pub(crate) struct Gravity {
    pub(crate) flags: GravityFlags,
    pub(crate) name: String,
    pub geom: Rectangle,
}

impl Gravity {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `name` - Name of this gravity
    /// * `x` - X percentage (0-199)
    /// * `y` - Y percentage (0-100)
    /// * `width` - Width percentage (0-100)
    /// * `height` - Height percentage (0-100)
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Gravity`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(name: &str, x: u16, y: u16, width: u16, height: u16) -> Self {
        let grav = Gravity {
            name: name.into(),
            geom: Rectangle {
                x: clamp!(x as i16, 0, 100),
                y: clamp!(y as i16, 0, 100),
                width: clamp!(width, 1, 100),
                height: clamp!(height, 1, 100),
            },
            ..Self::default()
        };
        
        debug!("{}: {}", function_name!(), grav);
        
        grav
    }

    pub(crate) fn apply_size(&self, bounds: &Rectangle, geom: &mut Rectangle) {
        geom.x = bounds.x + (bounds.width as i16 * self.geom.x / 100);
        geom.y = bounds.y + (bounds.height as i16 * self.geom.y / 100);
        geom.width = (bounds.width as u32 * self.geom.width as u32 / 100) as u16;
        geom.height = (bounds.height as u32 * self.geom.height as u32 / 100) as u16;
    }
}

impl fmt::Display for Gravity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(name={}, geom=(x={}, y={}, width={}, height={}))",
               self.name, self.geom.x, self.geom.y, self.geom.width, self.geom.height)
    }
}

/// Check config and init all gravity related options
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
    for gravity_values in config.gravities.iter() {
        if let (Some(MixedConfigVal::S(name)), Some(MixedConfigVal::I(x)),
            Some(MixedConfigVal::I(y)), Some(MixedConfigVal::I(width)),
            Some(MixedConfigVal::I(height))) = (gravity_values.get("name"), gravity_values.get("x"),
                                                gravity_values.get("y"), gravity_values.get("width"), gravity_values.get("height"))
        {
            subtle.gravities.push(Gravity::new(name, *x as u16, *y as u16, *width as u16, *height as u16));
        }
    }

    // Check gravities
    if 0 == subtle.gravities.len() {
        return Err(anyhow!("No gravities found"));
    }

    // Find default gravity
    if let Some(MixedConfigVal::S(grav_name)) = config.subtle.get("default_gravity") {
        if let Some(grav_id) = subtle.gravities.iter().position(|grav| grav.name.eq(grav_name)) {
            subtle.default_gravity = grav_id as isize;
        } else {
            subtle.default_gravity = 0;
        }
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
    
    let mut gravities: Vec<String> = Vec::with_capacity(subtle.gravities.len());

    for gravity in subtle.gravities.iter() {
        gravities.push(format!("{}x{}+{}+{}#{}", gravity.geom.x, gravity.geom.y,
                               gravity.geom.width, gravity.geom.height, gravity.name));
    }

    conn.change_property8(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_GRAVITY_LIST,
                          AtomEnum::STRING, gravities.join("\0").as_bytes())?.check()?;

    debug!("{}: ngravities={}", function_name!(), subtle.gravities.len());

    Ok(())
}
