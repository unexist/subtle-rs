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
use bitflags::{bitflags, bitflags_match};
use log::{debug, warn};
use anyhow::{anyhow, Result};
use easy_min_max::max;
use stdext::function_name;
use crate::client::{Client, ClientFlags};
use crate::config::Config;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct PanelFlags: u32 {
        const SUBLET = 1 << 0;      // Panel sublet type
        const COPY = 1 << 1;        // Panel copy type
        const VIEWS = 1 << 2;       // Panel views type
        const TITLE = 1 << 3;       // Panel title type
        const KEYCHAIN = 1 << 4;    // Panel keychain type
        const TRAY = 1 << 5;        // Panel tray type
        const ICON = 1 << 6;        // Panel icon type

        const SPACER1 = 1 << 7;     // Panel spacer1
        const SPACER2 = 1 << 8;     // Panel spacer2
        const SEPARATOR1 = 1 << 9;  // Panel separator1
        const SEPARATOR2 = 1 << 10; // Panel separator2
        const BOTTOM = 1 << 11;     // Panel bottom
        const HIDDEN = 1 << 12;     // Panel hidden
        const CENTER = 1 << 13;     // Panel center
        const SUBLETS = 1 << 14;    // Panel sublets

        const MOUSE_DOWN = 1 << 15;       // Panel mouse down
        const MOUSE_OVER = 1 << 16;       // Panel mouse over
        const MOUSE_OUT = 1 << 17;        // Panel mouse out
    }
}

#[derive(Default, Debug)]
pub(crate) struct Panel {
    pub(crate) flags: PanelFlags,
    pub(crate) x: i16,
    pub(crate) width: u16,
    pub(crate) screen_id: usize,
}

#[doc(hidden)]
fn format_client_modes(client: &Client) -> Result<String> {
    let mut x = 0;
    let mut mode_str =  [0; 6];

    // Collect window modes
    if client.flags.contains(ClientFlags::MODE_FULL) {
        mode_str[x] = '+' as u8;
        x += 1;
    }
    if client.flags.contains(ClientFlags::MODE_FLOAT) {
        mode_str[x] = '^' as u8;
        x += 1;
    }
    if client.flags.contains(ClientFlags::MODE_STICK) {
        mode_str[x] = '*' as u8;
        x += 1;
    }
    if client.flags.contains(ClientFlags::MODE_RESIZE) {
        mode_str[x] = '-' as u8;
        x += 1;
    }
    if client.flags.contains(ClientFlags::MODE_ZAPHOD) {
        mode_str[x] = '=' as u8;
        x += 1;
    }
    if client.flags.contains(ClientFlags::MODE_FIXED) {
        mode_str[x] = '!' as u8;
        x += 1;
    }

    String::from_utf8(mode_str[0..x].to_vec()).map_err(|e| anyhow!(e))
}

impl Panel {
    pub(crate) fn new(subtle: &Subtle, flag: PanelFlags) -> Self {
        let mut panel = Self {
            flags: flag,
            ..Self::default()
        };

        bitflags_match!(flag, {
            PanelFlags::ICON => {}, // TODO icon
            PanelFlags::SUBLETS => {}, // TODO sublets
            PanelFlags::VIEWS => {
                panel.flags.insert(PanelFlags::MOUSE_DOWN);
            },
            _ => warn!("Unknown panel flag {:?}", flag),
        });

        debug!("{}: {}", function_name!(), panel);

        panel
    }

    pub(crate) fn update(&mut self, subtle: &Subtle) -> Result<()> {

        // Handle panel item type
        if self.flags.contains(PanelFlags::TRAY) {
            // TODO tray
        } else if self.flags.contains(PanelFlags::ICON) {
            // TODO icon
        } else if self.flags.contains(PanelFlags::SUBLETS) {
            // TODO sublets
        } else if self.flags.contains(PanelFlags::TITLE) {
            self.width = subtle.clients_style.min_width as u16;

            // Find focus window
            if let Some(focus) = subtle.find_focus_client() {
                if !focus.is_alive() {
                    return Ok(());
                }

                if !focus.flags.contains(ClientFlags::TYPE_DESKTOP) {
                    if let Ok(mode_str) = format_client_modes(&*focus) {


                    }

                    // Ensure min-width
                    self.width = max!(subtle.clients_style.min_width as u16, self.width);
                }
            }
        } else if self.flags.contains(PanelFlags::VIEWS) {

        }

        debug!("{}: {}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x={}, width={}, screen_id={})", self.x, self.width, self.screen_id)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(subtle: &Subtle) -> Result<()> {
    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn update(subtle: &Subtle) {
    debug!("{}", function_name!());
}


pub(crate) fn render(subtle: &Subtle) {
    debug!("{}", function_name!());
}

pub(crate) fn publish(subtle: &Subtle, publish_all: bool) -> Result<()> {
    debug!("{}: panels={}", function_name!(), subtle.panels.len());

    Ok(())
}

