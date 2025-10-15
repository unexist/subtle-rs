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
use strum_macros::FromRepr;
use x11rb::{CURRENT_TIME, NONE};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, EventMask, PropMode, SetMode, StackMode, Window};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::ewmh;
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
#[derive(Copy, Clone, FromRepr)]
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

const XEMBED_MAPPED: u8 = 1 << 0; ///< Tray mapped

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
        tray.set_wm_protocols(subtle)?;
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

    pub(crate) fn set_wm_protocols(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let protocols = conn.get_property(false, self.win, atoms.WM_PROTOCOLS,
                                          AtomEnum::ATOM, 0, u32::MAX)?.reply()?.value;

        for protocol in protocols {
            if atoms.WM_DELETE_WINDOW == protocol as u32 {
                self.flags.insert(TrayFlags::CLOSE);
            }
        }

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn set_state(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();
        let mut opcode: XEmbed = XEmbed::WindowActivate;

        let xembed_info = conn.get_property(false, self.win, atoms._XEMBED_INFO,
            atoms._XEMBED_INFO, 0, 2)?.reply()?.value;

        println!("xembed_info={:?}", xembed_info);

        if let Some(xembed_flags) = xembed_info.first() {
            opcode = XEmbed::WindowActivate;

            conn.map_window(self.win)?.check()?;
            self.set_wm_state(subtle, WMState::Normal)?;
        } else {
            self.flags.insert(TrayFlags::UNMAP);

            opcode = XEmbed::WindowDeactivate;

            conn.unmap_window(self.win)?.check()?;
            self.set_wm_state(subtle, WMState::Withdrawn)?;

        }

        ewmh::send_message(subtle, self.win, atoms._XEMBED, &[CURRENT_TIME,
            opcode as u32, 0, 0, 0])?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn close(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        // Honor window preferences (see ICCCM 4.1.2.7, 4.2.8.1)
        if self.flags.intersects(TrayFlags::CLOSE) {
            ewmh::send_message(subtle, self.win, atoms.WM_PROTOCOLS,
                               &[atoms.WM_DELETE_WINDOW, CURRENT_TIME, 0, 0, 0])?;
        } else {
            // Kill it manually
            conn.kill_client(self.win)?.check()?;

            subtle.remove_tray_by_win(self.win);

            self.kill(subtle)?;

            publish(subtle)?;
        }

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn kill(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        // Remove _NET_WM_STATE (see EWMH 1.3)
        conn.delete_property(self.win, atoms._NET_WM_STATE)?.check()?;

        // Ignore further events
        conn.change_window_attributes(self.win, &ChangeWindowAttributesAux::default()
            .event_mask(EventMask::NO_EVENT))?.check()?;

        // Um-embed tray icon following XEmbed specs
        conn.unmap_window(self.win)?.check()?;

        let default_screen = &conn.setup().roots[subtle.screen_num];

        conn.reparent_window(self.win, default_screen.root, 0, 0)?.check()?;
        conn.map_window(self.win)?.check()?;
        conn.configure_window(self.win, &ConfigureWindowAux::default()
            .stack_mode(StackMode::TOP_IF))?.check()?;

        debug!("{}: client={}", function_name!(), self);

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

pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    let trays = subtle.trays.borrow();
    let mut wins: Vec<u32> = Vec::with_capacity(trays.len());

    // Sort clients from top to bottom
    for (tray_idx, tray) in trays.iter().enumerate() {
        wins.push(tray.win);
    }

    // EWMH: Client list and stacking list (same for us)
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_TRAY_LIST,
                           AtomEnum::WINDOW, &wins)?;

    conn.flush()?;

    debug!("{}: ntrays={}", function_name!(), trays.len());

    Ok(())
}
