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
use anyhow::Result;
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt;
use crate::Config;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const HORZ = 1 << 0; // Gravity tile gravity horizontally
        const VERT = 1 << 1; // Gravity tile gravity vertically
    }
}

#[derive(Default)]
pub(crate) struct Gravity {
    pub(crate) flags: Flags,
    pub(crate) name: String,
    pub geom: Rectangle,
}

impl Gravity {
    pub(crate) fn new(name: String, x: u16, y: u16, width: u16, height: u16) -> Self {
        let grav = Gravity {
            name,
            geom: Rectangle {
                x: clamp!(x as i16, 0, 100),
                y: clamp!(y as i16, 0, 100),
                width: clamp!(width, 1, 100),
                height: clamp!(height, 1, 100),
            },
            ..Self::default()
        };
        
        debug!("{}: {}", grav, function_name!());
        
        grav
    }
}

impl fmt::Display for Gravity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, geom=(x={}, y={}, width={}, height={})",
               self.name, self.geom.x, self.geom.y, self.geom.width, self.geom.height)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    subtle.gravities = config.gravities.iter()
        .map(|grav| Gravity::new(String::from(grav.0), grav.1[0], grav.1[1], 
                                 grav.1[2], grav.1[3])).collect();
    
    publish(subtle)?;

    debug!("{}", function_name!());
    
    Ok(())
}

pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];
    
    let mut gravities: Vec<String> = Vec::with_capacity(subtle.gravities.len());

    for gravity in subtle.gravities.iter() {
        gravities.push(format!("{}x{}+{}+{}#{}", gravity.geom.x, gravity.geom.y,
                               gravity.geom.width, gravity.geom.height, gravity.name));
    }

    conn.change_property8(PropMode::REPLACE, screen.root, atoms.SUBTLE_GRAVITY_LIST,
                          AtomEnum::STRING, gravities.join("\0").as_bytes())?.check()?;

    debug!("{}: gravities={}", function_name!(), subtle.gravities.len());

    Ok(())
}

