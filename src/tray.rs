///
/// @package subtle-rs
///
/// @file Tray functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use anyhow::Result;
use log::debug;
use stdext::function_name;
use x11rb::{CURRENT_TIME, NONE};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, PropMode, SetMode, Window};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::ewmh::WMState;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct TrayFlags: u32 {
        const DEAD = 1 << 0;  // Dead window
        const CLOSE = 1 << 1; // Send close message
        const UNMAP = 1 << 2; // Ignore unmaps
    }
}

#[derive(Default, Debug)]
pub(crate) struct Tray {
    pub(crate) flags: TrayFlags,

    pub(crate) win: Window,
    pub(crate) name: String,
    pub(crate) width: u16,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum XEmbed {
    EmbeddedNotify = 0, // Start embedding
    WindowActivate = 1, // Tray has focus
    WindowDeactivate = 2, // Tray has no focus
    RequestFocus = 3,
    FocusIn = 4, // Focus model
    FocusOut = 5,
    FocusNext = 6,
    FocusPrev = 7,
    GrabKey = 8,
    UngrabKey = 9,
    ModalityOn = 10,
    ModalityOff = 11,
    RegisterAccelerator = 12,
    UnregisterAccelerator = 13,
    ActivateAccelerator = 14,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum XEmbedFocus {
    Current = 0, // Focus default
    First = 1,
    Last = 2,
}

const XEMBED_MAPPED: i32 = 1 << 0; ///< Tray mapped

impl Tray {
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;

        // X Properties
        let geom_reply = conn.get_geometry(win)?.reply()?;

        let aux = ChangeWindowAttributesAux::default()
            .border_pixel(subtle.clients_style.bg as u32)
            .event_mask(EventMask::SUBSTRUCTURE_NOTIFY
                | EventMask::PROPERTY_CHANGE
                | EventMask::FOCUS_CHANGE
                | EventMask::ENTER_WINDOW);

        conn.change_window_attributes(win, &aux)?.check()?;
        conn.reparent_window(win, subtle.tray_win, 0, 0)?.check()?;

        conn.ungrab_server()?;

        let mut tray = Self {
            win,

            ..Self::default()
        };

        // Update client
        tray.set_wm_name(subtle)?;
        tray.set_wm_state(subtle, WMState::Withdrawn)?;

        // Start embedding life cycle
        conn.change_property32(PropMode::REPLACE, tray.win, atoms._XEMBED,
                               AtomEnum::CARDINAL, &[0xFFFFFF, CURRENT_TIME,
                XEmbed::EmbeddedNotify as u32, subtle.tray_win, 0])?.check()?;

        debug!("{}: tray={}", function_name!(), tray);

        Ok(tray)
    }

    pub(crate) fn set_wm_name(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let wm_name = conn.get_property(false, self.win,
                                        atoms.WM_NAME, AtomEnum::STRING,
                                        0, u32::MAX)?.reply()?.value;
        self.name = String::from_utf8(wm_name)?;

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }


    pub(crate) fn set_wm_state(&self, subtle: &Subtle, state: WMState) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let data: [u8; 2] = [state as u8, NONE as u8];

        conn.change_property(PropMode::REPLACE,
                             self.win, atoms.WM_STATE, atoms.WM_STATE, 8, 2, &data)?;

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for Tray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, win={}, width={}", self.name, self.win, self.width)
    }
}

impl PartialEq for Tray {
    fn eq(&self, other: &Self) -> bool {
        self.win == other.win
    }
}