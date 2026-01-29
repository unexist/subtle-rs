///
/// @package subtle-rs
///
/// @file Tray functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use anyhow::Result;
use easy_min_max::max;
use log::debug;
use stdext::function_name;
use strum_macros::FromRepr;
use x11rb::{CURRENT_TIME, NONE};
use x11rb::connection::Connection;
use x11rb::properties::{WmSizeHints, WmSizeHintsSpecification};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, EventMask, PropMode, SetMode, StackMode, Window};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::ewmh;
use crate::ewmh::WMState;
use crate::style::CalcSpacing;
use crate::subtle::Subtle;

bitflags! {
    /// Config and state-flags for [`Tray`]
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct TrayFlags: u32 {
        /// Dead window
        const DEAD = 1 << 0;
        /// Send close message
        const CLOSE = 1 << 1;
        /// Ignore unmaps
        const UNMAP = 1 << 2;
    }
}

#[derive(Default, Debug)]
pub(crate) struct Tray {
    /// Config and state-flags
    pub(crate) flags: TrayFlags,
    /// Tray win
    pub(crate) win: Window,
    /// Name of the tray
    pub(crate) name: String,
    /// Width of the win
    pub(crate) width: u16,
}

#[repr(u8)]
#[derive(Copy, Clone, FromRepr)]
pub(crate) enum XEmbed {
    /// Start embedding
    EmbeddedNotify = 0,
    /// Tray has focus
    WindowActivate = 1,
    /// Tray has no focus
    WindowDeactivate = 2,
    RequestFocus = 3,
    /// Focus model
    FocusIn = 4,
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
    /// Focus default
    Current = 0,
    First = 1,
    Last = 2,
}

/// Tray mapped
const XEMBED_MAPPED: u8 = 1 << 0;

impl Tray {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `win` - Tray window
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Tray`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;

        // X Properties
        let _geom_reply = conn.get_geometry(win)?.reply()?;

        let aux = ChangeWindowAttributesAux::default()
            .event_mask(EventMask::STRUCTURE_NOTIFY
                | EventMask::SUBSTRUCTURE_NOTIFY
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
        tray.set_size_hints(subtle)?;
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

    /// Set size hints for the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_size_hints(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Set default values
        self.width = 0;

        // Size hints - no idea why it's called normal hints
        if let Some(size_hints) = WmSizeHints::get_normal_hints(conn, self.win)?.reply()? {

            // Program min size - limit min size to screen size if larger
            if let Some((min_width, _)) = size_hints.min_size {
                self.width = max!(min_width as u16, subtle.panel_height * 2);
            }

            // Base sizes
            if let Some((base_width, _)) = size_hints.base_size {
                self.width = max!(base_width as u16, subtle.panel_height * 2);
            }

            // User/program size
            if let Some((hint_spec, x, _y)) = size_hints.size {
                match hint_spec {
                    WmSizeHintsSpecification::UserSpecified | WmSizeHintsSpecification::ProgramSpecified => {
                        self.width = max!(x as u16, subtle.panel_height * 2);
                    }
                }
            }
        }

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    /// Set WM_NAME for the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

    /// Set WM_STATE for the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `state` - New state
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_wm_state(&self, subtle: &Subtle, state: WMState) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let data: [u8; 2] = [state as u8, NONE as u8];

        conn.change_property(PropMode::REPLACE,
                             self.win, atoms.WM_STATE, atoms.WM_STATE, 8, 2, &data)?;

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    /// Set protocols for the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

    /// Resize underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `width` - New width
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn resize(&self, subtle: &Subtle, width: i32) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        conn.map_window(self.win)?.check()?;

        let aux = &ConfigureWindowAux::default()
            .x(width)
            .y(0i32)
            .width(max!(1, width) as u32)
            .height(max!(1, subtle.panel_height as i16
                            - subtle.tray_style.calc_spacing(CalcSpacing::Height)) as u32)
            .stack_mode(StackMode::ABOVE);

        conn.configure_window(self.win, &aux)?.check()?;

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    /// Set XEmbed state for the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_state(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();
        let mut opcode: XEmbed = XEmbed::WindowActivate;

        let xembed_info = conn.get_property(false, self.win, atoms._XEMBED_INFO,
            atoms._XEMBED_INFO, 0, 2)?.reply()?.value;

        if let Some(_xembed_flags) = xembed_info.first() {
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

        debug!("{}: tray={}", function_name!(), self);

        Ok(())
    }

    /// Close underlying win and honor ICCCM (ask or force)
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

    /// Kill the underlying win
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn kill(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Ignore further events
        conn.change_window_attributes(self.win, &ChangeWindowAttributesAux::default()
            .event_mask(EventMask::NO_EVENT))?;

        // Um-embed tray icon following XEmbed specs
        conn.unmap_window(self.win)?;

        let default_screen = &conn.setup().roots[subtle.screen_num];

        conn.reparent_window(self.win, default_screen.root, 0, 0)?;
        conn.map_window(self.win)?;
        conn.configure_window(subtle.tray_win, &ConfigureWindowAux::default()
            .stack_mode(StackMode::ABOVE))?;

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
