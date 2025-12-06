///
/// @package subtle-rs
///
/// @file Client functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use std::cmp::{Ordering, PartialEq};
use std::ops::{BitAnd, BitOr, BitXor};
use std::cell::Ref;
use x11rb::protocol::xproto::{Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, GrabMode, InputFocus, PropMode, QueryPointerReply, Rectangle, SetMode, StackMode, Window, CLIENT_MESSAGE_EVENT};
use bitflags::bitflags;
use anyhow::{anyhow, Context, Result};
use easy_min_max::max;
use log::debug;
use stdext::function_name;
use strum_macros::FromRepr;
use x11rb::connection::Connection;
use x11rb::{CURRENT_TIME, NONE};
use x11rb::properties::{WmHints, WmSizeHints, WmSizeHintsSpecification};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::{ewmh, grab, screen};
use crate::ewmh::{EWMHStateFlags, WMState};
use crate::grab::{DirectionOrder, GrabFlags};
use crate::subtle::{Subtle, SubtleFlags};
use crate::gravity::GravityFlags;
use crate::screen::{Screen, ScreenFlags};
use crate::tagging::Tagging;

const MIN_WIDTH: u16 = 1;
const MIN_HEIGHT: u16 = 1;

macro_rules! ignore_if_dead {
    ($client:tt) => {
        if $client.flags.contains(ClientFlags::DEAD) { return Ok(()); }
    };
}

#[repr(u8)]
#[derive(Default, Debug, Copy, Clone, PartialEq, FromRepr)]
pub(crate) enum RestackOrder {
    #[default]
    None = 0,
    Down = 1,
    Up = 2,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum DragMode {
    MOVE = 0,
    RESIZE = 1,
}


bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct DragEdge: u32 {
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
        const TOP = 1 << 2;
        const BOTTOM = 1 << 3;
    }
}

bitflags! {
    /// Config and state-flags for [`Client`]
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct ClientFlags: u32 {
        /// Dead window
        const DEAD = 1 << 0;
        /// Send focus message
        const FOCUS = 1 << 1;
        /// Active/passive focus-model
        const INPUT = 1 << 2;
        /// Send close message
        const CLOSE = 1 << 3;
        /// Ignore unmaps
        const UNMAP = 1 << 4;
        /// Re-arrange client
        const ARRANGE = 1 << 5;

        /// Fullscreen mode (also used in tags)
        const MODE_FULL = 1 << 6;
        /// Float mode
        const MODE_FLOAT = 1 << 7;
        /// Stick mode
        const MODE_STICK = 1 << 8;
        /// Stick tagged screen mode
        const MODE_STICK_SCREEN = 1 << 9;
        /// Urgent mode
        const MODE_URGENT = 1 << 10;
        /// Resize mode
        const MODE_RESIZE = 1 << 11;
        /// Zaphod mode
        const MODE_ZAPHOD = 1 << 12;
        /// Fixed size mode
        const MODE_FIXED = 1 << 13;
        /// Center position mode
        const MODE_CENTER = 1 << 14;
        /// Borderless
        const MODE_BORDERLESS = 1 << 15;

        /// Normal type (also used in match)
        const TYPE_NORMAL = 1 << 16;
        /// Desktop type
        const TYPE_DESKTOP = 1 << 17;
        /// Dock type
        const TYPE_DOCK = 1 << 18;
        /// Toolbar type
        const TYPE_TOOLBAR = 1 << 19;
        /// Splash type
        const TYPE_SPLASH = 1 << 20;
        /// Dialog type
        const TYPE_DIALOG = 1 << 21;

        /// Catch all for modes
        const ALL_MODES = Self::MODE_FULL.bits() | Self::MODE_FLOAT.bits()
            | Self::MODE_STICK.bits() | Self::MODE_STICK_SCREEN.bits()
            | Self::MODE_URGENT.bits() | Self::MODE_RESIZE.bits()
            | Self::MODE_ZAPHOD.bits() | Self::MODE_FIXED.bits()
            | Self::MODE_CENTER.bits() | Self::MODE_BORDERLESS.bits();
    }
}

#[derive(Default, Debug)]
pub(crate) struct Client {
    pub(crate) flags: ClientFlags,
    pub(crate) tags: Tagging,

    pub(crate) win: Window,
    pub(crate) leader: Window,

    pub(crate) name: String,
    pub(crate) instance: String,
    pub(crate) klass: String,
    pub(crate) role: String,

    pub(crate) min_ratio: f32,
    pub(crate) max_ratio: f32,

    pub(crate) min_width: u16,
    pub(crate) min_height: u16,
    pub(crate) max_width: i16,
    pub(crate) max_height: i16,
    pub(crate) width_inc: u16,
    pub(crate) height_inc: u16,
    pub(crate) base_width: u16,
    pub(crate) base_height: u16,

    pub(crate) screen_idx: isize,
    pub(crate) gravity_idx: isize,
    
    pub(crate) geom: Rectangle,
    pub(crate) order: RestackOrder,

    pub(crate) gravities: Vec<usize>,
}

impl Client {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `win` - Client win
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Client`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;

        // X Properties
        let geom_reply = conn.get_geometry(win)?.reply()?;

        let aux = ChangeWindowAttributesAux::default()
            .border_pixel(subtle.clients_style.bg as u32)
            .event_mask(EventMask::PROPERTY_CHANGE
                | EventMask::FOCUS_CHANGE
                | EventMask::ENTER_WINDOW);

        conn.change_window_attributes(win, &aux)?.check()?;

        let aux = ConfigureWindowAux::default()
            .border_width(subtle.clients_style.border.top as u32);

        conn.configure_window(win, &aux)?.check()?;

        conn.ungrab_server()?;

        let mut client = Self {
            flags: ClientFlags::INPUT,
            win,

            screen_idx: 0,
            gravity_idx: -1,

            geom: Rectangle {
                x: geom_reply.x,
                y: geom_reply.y,
                width: max!(MIN_WIDTH, geom_reply.width),
                height: max!(MIN_HEIGHT, geom_reply.height),
            },
            gravities: Vec::with_capacity(subtle.views.len()),
            ..Self::default()
        };

        // Init gravities
        let grav = get_default_gravity(subtle);

        for i in 0..subtle.views.len() {
            client.gravities.push(grav as usize);
        }

        // Update client
        let mut mode_flags = ClientFlags::empty();

        //client.set_strut(subtle)?;
        client.set_size_hints(subtle, &mut mode_flags)?;
        client.set_wm_name(subtle)?;
        client.set_wm_state(subtle, WMState::Withdrawn)?;
        client.set_wm_protocols(subtle)?;
        client.set_wm_type(subtle, &mut mode_flags)?;
        client.set_wm_hints(subtle, &mut mode_flags)?;
        client.set_motif_wm_hints(subtle, &mut mode_flags)?;
        client.set_net_wm_state(subtle, &mut mode_flags)?;
        client.set_transient(subtle, &mut mode_flags)?;
        client.retag(subtle, &mut mode_flags)?;
        client.toggle(subtle, &mut mode_flags, false)?;

        // Set leader window
        let leader = conn.get_property(false, client.win, atoms.WM_CLIENT_LEADER,
                                       AtomEnum::WINDOW, 0, 1)?.reply()?.value;

        if !leader.is_empty() && NONE != leader[0] as u32 {
            client.leader = leader[0] as Window;
        }

        // EWMH: Gravity, screen, desktop, extents
        let data: [u32; 1] = [client.gravity_idx as u32];

        conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_GRAVITY,
            AtomEnum::CARDINAL, &data)?.check()?;

        let data: [u32; 1] = [client.screen_idx as u32];

        conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_SCREEN,
                               AtomEnum::CARDINAL, &data)?.check()?;

        let data: [u32; 1] = [0];

        conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_WM_DESKTOP,
            AtomEnum::CARDINAL, &data)?.check()?;

        // TODO Struts
        //conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_FRAME_EXTENTS
        //                       AtomEnum::CARDINAL, &data)?.check()?;

        debug!("{}: client={}", function_name!(), client);

        Ok(client)
    }

    /// Set and evaluate strut values for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_strut(&mut self, subtle: &mut Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let reply = conn.get_property(false, self.win, AtomEnum::CARDINAL,
                                      atoms._NET_WM_STRUT, 0, 4)?.reply()?;

        if 4 == reply.value.len() {
            subtle.clients_style.padding.left = max!(subtle.clients_style.padding.left,
                reply.value[0] as i16);
            subtle.clients_style.padding.right = max!(subtle.clients_style.padding.right,
                reply.value[1] as i16);
            subtle.clients_style.padding.top = max!(subtle.clients_style.padding.top,
                reply.value[2] as i16);
            subtle.clients_style.padding.bottom = max!(subtle.clients_style.padding.bottom,
                reply.value[3] as i16);

            // Update screen and clients
            screen::resize(subtle)?;
            screen::configure(subtle)?;
        }


        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Set and evaluate size hints values for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_size_hints(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Assume first screen
        let screen = subtle.screens.first().context("No screens")?;

        // Set default values
        self.min_width = MIN_WIDTH;
        self.min_height = MIN_HEIGHT;
        self.max_width = -1;
        self.max_height = -1;
        self.min_ratio = 0.0;
        self.max_ratio = 0.0;
        self.width_inc = 1;
        self.height_inc = 1;
        self.base_width = 0;
        self.base_height = 0;

        // Size hints - no idea why it's called normal hints
        if let Some(size_hints) = WmSizeHints::get_normal_hints(conn, self.win)?.reply()? {

            // Program min size - limit min size to screen size if larger
           if let Some((min_width, min_height)) = size_hints.min_size {
               self.min_width = if self.min_width > screen.geom.width {
                   screen.geom.width } else { max!(MIN_WIDTH, min_width as u16) };

               self.min_height = if self.min_height > screen.geom.height {
                   screen.geom.height } else { max!(MIN_HEIGHT, min_height as u16) };
           }

            // Program max size - limit max size to screen if larger
            if let Some((max_width, max_height)) = size_hints.max_size {
                self.max_width = if max_width > screen.geom.width as i32 {
                    screen.geom.width as i16 } else { max_width as i16 };

                self.max_height = if max_height > screen.geom.height as i32 {
                    screen.geom.height as i16 - subtle.panel_height as i16
                } else { max_height as i16 };
            }

            // Set float when min == max size (EWMH: Fixed size windows)
            if let Some((min_width, min_height)) = size_hints.min_size
                && let Some((max_width, max_height)) = size_hints.max_size
            {
                if min_width == max_width && min_height == max_height && !self.flags.contains(ClientFlags::TYPE_DESKTOP) {
                    mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FIXED);
                }
            }

            // Aspect ratios
            if let Some((min_aspect, max_aspect)) = size_hints.aspect {
                self.min_ratio = min_aspect.numerator as f32 / min_aspect.denominator as f32;
                self.max_ratio = max_aspect.numerator as f32 / max_aspect.denominator as f32;
            }

            // Resize increment steps
            if let Some((width_inc, height_inc)) = size_hints.size_increment {
                self.width_inc = width_inc as u16;
                self.height_inc = height_inc as u16;

            }

            // Base sizes
            if let Some((base_width, base_height)) = size_hints.base_size {
                self.base_width = base_width as u16;
                self.base_height = base_height as u16;
            }

            // Check for specific position and size
            if subtle.flags.contains(SubtleFlags::RESIZE)
                || self.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE | ClientFlags::TYPE_DOCK)
            {
                // User/program position
                if let Some((hint_spec, x, y)) = size_hints.position {
                    match hint_spec {
                        WmSizeHintsSpecification::UserSpecified | WmSizeHintsSpecification::ProgramSpecified => {
                            self.geom.x = x as i16;
                            self.geom.y = y as i16;
                        }
                    }
                }

                // User/program size
                if let Some((hint_spec, x, y)) = size_hints.size {
                    match hint_spec {
                        WmSizeHintsSpecification::UserSpecified | WmSizeHintsSpecification::ProgramSpecified => {
                            self.geom.width = x as u16;
                            self.geom.height = y as u16;
                        }
                    }
                }

                // Sanitize positions for stupid clients like GIMP
                self.resize(subtle, &screen.geom, true)?;
            }
        }

        debug!("{}: client={}, minw={}, minh={}, maxw={}, maxh={}, \
            minr={}, maxr={}, incw={}, inch={}, basew={}, baseh={}",
            function_name!(), self, self.min_width, self.min_height,
            self.max_width, self.max_height,
            self.min_ratio, self.max_ratio, self.width_inc, self.height_inc,
            self.base_width, self.base_height);

        Ok(())
    }

    /// Set WM_NAME for client
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

        let wm_role= conn.get_property(false, self.win, AtomEnum::STRING,
                                       atoms.WM_WINDOW_ROLE, 0, u32::MAX)?.reply()?.value;

        let wm_klass = conn.get_property(false, self.win, atoms.WM_CLASS,
                                         AtomEnum::STRING, 0, u32::MAX)?.reply()?.value;


        let inst_klass = String::from_utf8(wm_klass)
            .expect("UTF-8 string should be valid UTF-8")
            .trim_matches('\0')
            .split('\0')
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        self.name = String::from_utf8(wm_name)?;
        self.role = String::from_utf8(wm_role)?;
        self.instance =  inst_klass[0].to_string();
        self.klass = inst_klass[1].to_string();

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Set WM_STATE for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `state` - The state to set
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

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Set and evaluate wm protocols for client
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
            if atoms.WM_TAKE_FOCUS == protocol as u32 {
                self.flags.insert(ClientFlags::FOCUS);
            } else if atoms.WM_DELETE_WINDOW == protocol as u32 {
                self.flags.insert(ClientFlags::CLOSE);
            }
        }

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Set wm type for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_wm_type(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let wm_types = conn.get_property(false, self.win, AtomEnum::ATOM,
                                         atoms._NET_WM_WINDOW_TYPE, 0, 5)?.reply()?.value;

        for wm_type in wm_types {
            if atoms._NET_WM_WINDOW_TYPE_DESKTOP == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DESKTOP);
                mode_flags.insert(ClientFlags::MODE_FIXED | ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_WINDOW_TYPE_DOCK == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DOCK);
                mode_flags.insert(ClientFlags::MODE_FIXED | ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_WINDOW_TYPE_TOOLBAR == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_TOOLBAR);
            } else if atoms._NET_WM_WINDOW_TYPE_SPLASH == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_SPLASH);
                mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_CENTER);
            } else if atoms._NET_WM_WINDOW_TYPE_DIALOG == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DIALOG);
                mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_CENTER);
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Set and evaluate wm hints for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_wm_hints(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Window manager hints (ICCCM 4.1.7)
        if let Some(wm_hints) = WmHints::get(conn, self.win)?.reply()? {
            // Handle urgency hint:
            // Set urgency if window hasn't got focus and remove it after getting focus
            if wm_hints.urgent && let Some(focus_win) = subtle.focus_history.borrow(0)
                && focus_win != self.win
            {
                self.flags.remove(ClientFlags::MODE_URGENT);
            }

            // Handle window group hint
            if wm_hints.window_group.is_some() {
                if let Some(group_lead) = subtle.find_client(wm_hints.window_group.unwrap()) {
                    self.flags = group_lead.flags; // TODO *flags |= (k->flags & MODES_ALL);
                    self.tags = group_lead.tags;
                    self.screen_idx = group_lead.screen_idx;
                }
            }

            // Handle just false value of input hint since it is the default
            match wm_hints.input {
                Some(false) => self.flags.remove(ClientFlags::INPUT),
                _ => {}
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Set and evaluate _MOTIF_WM_HINTS for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_motif_wm_hints(&self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let hints = conn.get_property(false, self.win, atoms._MOTIF_WM_HINTS,
                                      atoms._MOTIF_WM_HINTS, 0, 1)?.reply()?.value;

        // TODO

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Set and evaluate _NET_WM_STATE for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_net_wm_state(&self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let states = conn.get_property(false, self.win, AtomEnum::ATOM,
                                       atoms._NET_WM_STATE, 0, 4)?.reply()?.value;

        for state in states {
            if atoms._NET_WM_STATE_FULLSCREEN == state as Atom {
                mode_flags.insert(ClientFlags::MODE_FULL);
            } else if atoms._NET_WM_STATE_ABOVE == state as Atom {
                mode_flags.insert(ClientFlags::MODE_FLOAT);
            } else if atoms._NET_WM_STATE_STICKY == state as Atom {
                mode_flags.insert(ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_STATE_DEMANDS_ATTENTION == state as Atom {
                mode_flags.insert(ClientFlags::MODE_URGENT);
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Set transient state for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn set_transient(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        let trans = conn.get_property(false, self.win, AtomEnum::WM_TRANSIENT_FOR,
                          AtomEnum::WINDOW, 0, 1)?.reply()?.value;

        if !trans.is_empty() {
            // Check if transient windows should be urgent
            mode_flags.insert(if subtle.flags.intersects(SubtleFlags::URGENT) {
                ClientFlags::MODE_FLOAT | ClientFlags::MODE_URGENT
            } else {
                ClientFlags::MODE_FLOAT
            });

            // Find parent window
            if let Some(parent) = subtle.find_client(trans[0] as Window) {
               mode_flags.insert(parent.flags & ClientFlags::ALL_MODES);

                self.tags.insert(parent.tags);
                self.screen_idx = parent.screen_idx;
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Set focus to client on active screen
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `warp_pointer` - Whether to move pointer to focus window
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn focus(&self, subtle: &Subtle, warp_pointer: bool) -> Result<()> {
        if !self.is_visible(subtle) {
            return Ok(());
        }

        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        // Unset current focus
        if let Some(win) = subtle.focus_history.borrow(0) && self.win != *win {
            if let Some(focus) = subtle.find_client(*win) {
                grab::unset(subtle, focus.win)?;

                // Reorder focus history
                // TODO

                if !focus.flags.contains(ClientFlags::TYPE_DESKTOP) {
                    let aux = ChangeWindowAttributesAux::default()
                        .border_pixel(subtle.clients_style.bg as u32);

                    conn.change_window_attributes(focus.win, &aux)?.check()?;
                }
            }
        }

        // Check client input focus type (see ICCCM 4.1.7, 4.1.2.7, 4.2.8)
        if !self.flags.contains(ClientFlags::INPUT) && self.flags.contains(ClientFlags::FOCUS) {
            conn.send_event(false, self.win, EventMask::NO_EVENT, ClientMessageEvent {
                response_type: CLIENT_MESSAGE_EVENT,
                format: 32,
                sequence: 0,
                window: self.win,
                type_: atoms.WM_PROTOCOLS,
                data: [atoms.WM_TAKE_FOCUS, CURRENT_TIME, 0, 0, 0].into(),
            })?.check()?;
        } else if self.flags.contains(ClientFlags::INPUT) {
            conn.set_input_focus(InputFocus::POINTER_ROOT, self.win, CURRENT_TIME)?.check()?;
        }

        // Update focus
        //subtle.focus_history.remove()
        grab::set(subtle, self.win, GrabFlags::IS_MOUSE)?;

        // Exclude desktop and dock type windows
        if !self.flags.intersects(ClientFlags::TYPE_DESKTOP | ClientFlags::TYPE_DOCK) {
            conn.change_window_attributes(self.win, &ChangeWindowAttributesAux::default()
                .border_pixel(subtle.clients_style.fg as u32))?.check()?;
        }

        // EWMH: Active window
        let default_screen = &conn.setup().roots[subtle.screen_num];

        let list = subtle.focus_history.inner().iter()
            .map(|elem| elem.get() as u32).collect::<Vec<_>>();

        conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_ACTIVE_WINDOW,
                               AtomEnum::WINDOW, list.as_slice())?.check()?;

        // Warp pointer
        if warp_pointer && !subtle.flags.intersects(SubtleFlags::SKIP_POINTER_WARP) {
            self.warp_pointer(subtle)?;
        }

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Toggle mode flags for client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to toggle for this type
    /// * `set_gravity` - Whether to also set gravity
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn toggle(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags, set_gravity: bool) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        // Set arrange for certain modes
        if mode_flags.intersects(ClientFlags::MODE_FLOAT | ClientFlags::MODE_STICK | ClientFlags::MODE_FULL
            | ClientFlags::MODE_ZAPHOD | ClientFlags::MODE_BORDERLESS | ClientFlags::MODE_CENTER)
        {
            self.flags.insert(ClientFlags::ARRANGE);
        }

        // Handle sticky mode
        if mode_flags.contains(ClientFlags::MODE_STICK) {
            // Unset stick mode
            if self.flags.contains(ClientFlags::MODE_STICK) {
                if self.flags.contains(ClientFlags::MODE_URGENT) {
                    //subtle.urgent_tags.remove(self.tags); // TODO urgent
                }
            } else {
                if set_gravity {
                    // Set gravity for untagged views
                    for (view_idx, view) in subtle.views.iter().enumerate() {
                        if !view.tags.contains(self.tags) && -1 != self.gravity_idx {
                            self.gravities[view_idx] = self.gravity_idx as usize;
                        }
                    }
                }

                // Set screen when required
                if !self.flags.contains(ClientFlags::MODE_STICK_SCREEN) {
                    // Find screen: Prefer screen of current window
                    if subtle.flags.contains(SubtleFlags::SKIP_POINTER_WARP)  {
                        if let Some(win) = subtle.focus_history.borrow(0) {
                            if let Some(focus) = subtle.find_client(*win) {
                                if focus.is_visible(subtle) {
                                    self.screen_idx = focus.screen_idx;
                                }
                            }
                        }
                    } else if let Some((idx, _)) = subtle.find_screen_by_pointer() {
                        self.screen_idx = idx as isize;
                    }
                }
            }
        }

        // Handle fullscreen mode
        if mode_flags.contains(ClientFlags::MODE_FULL) {
            if self.flags.contains(ClientFlags::MODE_FULL) {
                if !self.flags.contains(ClientFlags::MODE_BORDERLESS) {
                    let aux = ConfigureWindowAux::default()
                        .border_width(subtle.clients_style.border.top as u32);

                    conn.configure_window(self.win, &aux)?.check()?;
                }
            } else {
                // Normally, you'd expect that a fixed size window wants to keep the size.
                // Apparently, some broken clients just violate that, so we exclude fixed
                // windows with min != screen size from fullscreen
                if self.flags.contains(ClientFlags::MODE_FIXED) {
                    if let Some(screen) = subtle.screens.get(self.screen_idx as usize) {
                        if screen.base.width != self.min_width || screen.base.height != self.min_height {
                            mode_flags.remove(ClientFlags::MODE_FULL);
                        }
                    }
                }

                let aux = ChangeWindowAttributesAux::default()
                    .border_pixel(0);

                conn.change_window_attributes(self.win, &aux)?.check()?;
            }
        }

        // Handle borderless
        if mode_flags.contains(ClientFlags::MODE_BORDERLESS) {
            let mut aux = ConfigureWindowAux::default();

            // Unset borderless
            if !self.flags.contains(ClientFlags::MODE_BORDERLESS) {
                aux = aux.border_width(subtle.clients_style.border.top as u32);
            } else {
                aux = aux.border_width(0);
            }

            conn.configure_window(self.win, &aux)?.check()?;
        }

        // Handle urgent
        if mode_flags.contains(ClientFlags::MODE_URGENT) {
            //subtle.urgent_tags.insert(self.tags) // TODO urgent
        }

        // Handle center mode
        if mode_flags.contains(ClientFlags::MODE_CENTER) {
            if self.flags.contains(ClientFlags::MODE_CENTER) {
                self.flags.remove(ClientFlags::MODE_FLOAT);
                self.flags.insert(ClientFlags::ARRANGE);
            } else {
                if let Some(screen) = subtle.screens.get(self.screen_idx as usize) {
                    debug!("client={}, screen={}", self, screen);
                    // Set to screen center
                    self.geom.x = screen.geom.x + (screen.geom.width as i16 - self.geom.width as i16 - 2 * 1) / 2; // TODO BORDER
                    self.geom.y = screen.geom.y + (screen.geom.height as i16 - self.geom.height as i16 - 2 * 1) / 2; // TODO BORDER

                    mode_flags.insert(ClientFlags::MODE_FLOAT);
                    self.flags.insert(ClientFlags::ARRANGE);
                }
            }
        }

        // Handle desktop and dock type (one way)
        if mode_flags.contains(ClientFlags::TYPE_DESKTOP | ClientFlags::TYPE_DOCK) {
            let aux = ConfigureWindowAux::default()
                .border_width(0);

            conn.configure_window(self.win, &aux)?.check()?;

            // Special treatment
            if mode_flags.contains(ClientFlags::TYPE_DESKTOP) {
                if let Some(screen) = subtle.screens.get(self.screen_idx as usize) {
                    self.geom = screen.base;

                    // Add panel heights without struts
                    if screen.flags.contains(ScreenFlags::TOP_PANEL) {
                        self.geom.y += subtle.panel_height as i16;
                        self.geom.height -= subtle.panel_height;
                    }

                    if screen.flags.contains(ScreenFlags::BOTTOM_PANEL) {
                        self.geom.height -= subtle.panel_height;
                    }
                }
            }
        }

        // Finally toggle mode flags only
        // TODO  c->flags = ((c->flags & ~MODES_ALL) | ((c->flags & MODES_ALL) ^ (flags & MODES_ALL)));
        self.flags = self.flags.bitand(ClientFlags::ALL_MODES.complement())
            .bitor(self.flags.bitand(ClientFlags::ALL_MODES))
            .bitxor(mode_flags.bitand(ClientFlags::ALL_MODES));

        // Sort for keeping stacking order
        if self.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL
            | ClientFlags::TYPE_DESKTOP | ClientFlags::TYPE_DOCK)
        {
            restack_clients(RestackOrder::Up)?;
        }

        // EWMH: State and flags
        let mut state_atoms: Vec<Atom> = Vec::default();
        let mut ewmh_state = EWMHStateFlags::empty();

        if self.flags.contains(ClientFlags::MODE_FULL) {
            state_atoms.push(atoms._NET_WM_STATE_FULLSCREEN);
            ewmh_state.insert(EWMHStateFlags::FULL);
        }

        if self.flags.contains(ClientFlags::MODE_FLOAT) {
            state_atoms.push(atoms._NET_WM_STATE_ABOVE);
            ewmh_state.insert(EWMHStateFlags::FLOAT);
        }

        if self.flags.contains(ClientFlags::MODE_STICK) {
            state_atoms.push(atoms._NET_WM_STATE_STICKY);
            ewmh_state.insert(EWMHStateFlags::STICK);
        }

        if self.flags.contains(ClientFlags::MODE_URGENT) {
            state_atoms.push(atoms._NET_WM_STATE_DEMANDS_ATTENTION);
            ewmh_state.insert(EWMHStateFlags::URGENT);
        }

        conn.change_property32(PropMode::REPLACE, self.win, atoms._NET_WM_STATE,
                               AtomEnum::ATOM, state_atoms.as_slice())?.check()?;

        conn.change_property32(PropMode::REPLACE, self.win, atoms.SUBTLE_CLIENT_FLAGS,
                                AtomEnum::CARDINAL, &[ewmh_state.bits()])?.check()?;

        conn.flush()?;

        debug!("{}: client={}, mode_flags={:?}, gravity={}", function_name!(),
            self, mode_flags, set_gravity);

        Ok(())
    }

    /// Add tag given by idx to this client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `tag_idx` - Tag index
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]

    pub(crate) fn tag(&mut self, subtle: &Subtle, tag_idx: usize, mode_flags: &mut ClientFlags) -> Result<()> {
        ignore_if_dead!(self);

        // Update flags and tags
        if let Some(tag) = subtle.tags.get(tag_idx) {
            self.tags |= Tagging::from_bits_retain(1 << tag_idx);
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Re-add every matching tag to this client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn retag(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        for (tag_idx, tag) in subtle.tags.iter().enumerate() {
            if tag.matches(self) {
                self.tag(subtle, tag_idx, mode_flags)?;
            }
        }

        if self.flags.contains(ClientFlags::MODE_STICK) && !mode_flags.contains(ClientFlags::MODE_STICK) {
            let mut visible: u8 = 0;

            for view in subtle.views.iter() {
                if view.tags.contains(self.tags) {
                    visible += 1;
                }
            }

            if 0 == visible {
                self.tag(subtle,0, mode_flags)?;
            }
        }

        // EWMH: Tags
        let data: [u32; 1] = [self.tags.bits()];

        conn.change_property32(PropMode::REPLACE, self.win, 
                               atoms.SUBTLE_CLIENT_TAGS, AtomEnum::CARDINAL, &data)?.check()?;

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    /// Update and re-arrange this client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `mode_flags` - Mode flags to set for this type
    /// * `gravity_idx` - Gravity index
    /// * `screen_idx` - Screen index
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn arrange(&mut self, subtle: &Subtle, gravity_idx: isize, screen_idx: isize) -> Result<()> {
        ignore_if_dead!(self);

        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let screen = subtle.screens.get(screen_idx as usize)
            .context("Screen not found?")?;

        // Check flags
        if self.flags.intersects(ClientFlags::MODE_FULL) {
            let mut aux = ConfigureWindowAux::default();

            // Use all screens in zaphod mode
            if self.flags.contains(ClientFlags::MODE_ZAPHOD) {
                aux = aux.x(0)
                    .y(0)
                    .width(subtle.width as u32)
                    .height(subtle.height as u32)
                    .stack_mode(StackMode::ABOVE);
            } else if let Some(screen) = subtle.screens.get(self.screen_idx as usize) {
                aux = aux.x(screen.base.x as i32)
                    .y(screen.base.y as i32)
                    .width(screen.base.width as u32)
                    .height(screen.base.height as u32)
                    .stack_mode(StackMode::ABOVE);
            }

            conn.configure_window(self.win, &aux)?.check()?;
        } else if self.flags.intersects(ClientFlags::MODE_FLOAT) {
            if self.flags.intersects(ClientFlags::ARRANGE)
                || (-1 != screen_idx && self.screen_idx != screen_idx)
            {
                if let Some(old_screen) = subtle.screens.get(
                    (if -1 != self.screen_idx { self.screen_idx } else { 0 }) as usize)
                {
                    if screen_idx != self.screen_idx {
                        self.geom.x = self.geom.x - old_screen.geom.x + screen.geom.x;
                        self.geom.y = self.geom.y - old_screen.geom.y + screen.geom.y;
                        self.screen_idx = screen_idx;
                    }
                }

                // Finally resize window
                self.resize(subtle, &screen.geom, true)?;

                conn.configure_window(self.win, &ConfigureWindowAux::default()
                    .x(self.geom.x as i32)
                    .y(self.geom.y as i32)
                    .width(self.geom.width as u32)
                    .height(self.geom.height as u32))?.check()?;
            }
        } else if self.flags.intersects(ClientFlags::TYPE_DESKTOP | ClientFlags::TYPE_DOCK) {
            if self.flags.intersects(ClientFlags::TYPE_DESKTOP) {
                self.geom = screen.geom;
            }

            // Just use screen size for desktop windows
            conn.configure_window(self.win, &ConfigureWindowAux::default()
                .x(self.geom.x as i32)
                .y(self.geom.y as i32)
                .width(self.geom.width as u32)
                .height(self.geom.height as u32))?.check()?;

            //XLowerWindow() // TODO
        } else {
            if self.flags.intersects(ClientFlags::ARRANGE) || self.gravity_idx != gravity_idx
                || self.screen_idx != screen_idx
            {
                let old_gravity_id = self.gravity_idx;
                let old_screen_id = self.screen_idx;

                // Set values
                if -1 != screen_idx {
                    self.screen_idx = screen_idx;
                }

                if -1 != gravity_idx {
                    self.gravity_idx = gravity_idx;
                    self.gravities[screen.view_idx.get() as usize] = gravity_idx as usize;
                }

                // Gravity tiling
                let maybe_old_gravity = subtle.gravities.get(old_gravity_id as usize);

                if -1 != old_screen_id && (subtle.flags.contains(SubtleFlags::GRAVITY_TILING)
                    || maybe_old_gravity.is_some() &&
                    maybe_old_gravity.unwrap().flags.contains(GravityFlags::HORZ | GravityFlags::VERT))
                {
                    self.gravity_tile(subtle, old_gravity_id, old_screen_id)?;
                }

                let maybe_gravity = subtle.gravities.get(gravity_idx as usize);

                if subtle.flags.contains(SubtleFlags::GRAVITY_TILING)
                    && (maybe_gravity.is_some()
                    && maybe_gravity.unwrap().flags.contains(GravityFlags::HORZ | GravityFlags::VERT))
                {
                    self.gravity_tile(subtle, gravity_idx, if -1 == screen_idx { 0 } else { screen_idx })?;
                } else {
                    let mut bounds = screen.geom;

                    // Set size for bounds
                    if self.flags.contains(ClientFlags::MODE_ZAPHOD) {
                        calc_zaphod(subtle, &mut bounds)?;
                    }

                    if maybe_gravity.is_some() {
                        maybe_gravity.unwrap().apply_size(&bounds, &mut self.geom);
                    }

                    self.move_resize(subtle, &bounds)?;
                }
            }
        }

        // EWMH: Gravity
        conn.change_property32(PropMode::REPLACE, self.win, atoms.SUBTLE_CLIENT_GRAVITY,
                               AtomEnum::CARDINAL,&[self.gravity_idx as u32])?.check()?;

        conn.flush()?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Resize client window
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `bounds` - Outer bounds for sizes
    /// * `use_size_hints` - Whether to apply and honor size hints
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn resize(&mut self, subtle: &Subtle, bounds: &Rectangle, use_size_hints: bool) -> Result<()> {
        let mut geom = self.geom;

        if use_size_hints {
            self.apply_size_hints(subtle, bounds, false, false, &mut geom);
        }

        if !self.flags.contains(ClientFlags::MODE_FULL | ClientFlags::TYPE_DOCK) {
            let mut max_x = 0;
            let mut max_y = 0;

            if !self.flags.contains(ClientFlags::MODE_FIXED) {
                if geom.width > bounds.width {
                    geom.width = bounds.width;
                }

                if geom.height > bounds.height {
                    geom.height = bounds.height;
                }
            }

            // Check whether window fits into bounds
            max_x = bounds.x + bounds.width as i16;
            max_y = bounds.y + bounds.height as i16;

            // Check x and center
            if geom.x < bounds.x || geom.x > max_x || geom.x + geom.width as i16  > max_x {
                if self.flags.contains(ClientFlags::MODE_FLOAT) {
                    geom.x = bounds.x + ((bounds.width as i16 - geom.width as i16) / 2);
                } else {
                    geom.x = bounds.x;
                }
            }

            // Check y and center
            if geom.y < bounds.y || geom.y > max_y || geom.y + geom.height as i16 > max_y {
                if self.flags.contains(ClientFlags::MODE_FLOAT) {
                    geom.y = bounds.y + ((bounds.height as i16 - geom.height as i16) / 2);
                } else {
                    geom.y = bounds.y;
                }
            }
        }

        // Finally update geom
        self.geom = geom;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Sort and restack client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `order` - Sorting / restacking order
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn restack(&mut self, subtle: &Subtle, order: RestackOrder) -> Result<()> {
        self.order = order;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Snap window to outer bounds of screen
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `screen` - Screen to use
    /// * `geom` - Geometry to snap to screen bounds
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn snap(&self, subtle: &Subtle, screen: &Screen, geom: &mut Rectangle) -> Result<()> {
        ignore_if_dead!(self);

        // Snap to screen border when value is in snap margin - X axis
        if (screen.geom.x - geom.x).abs() <= subtle.snap_size as i16 {
            geom.x = screen.geom.x + self.get_border_width(subtle);
        } else if ((screen.geom.x + screen.geom.width as i16)
            - (geom.x + geom.width as i16 + self.get_border_width(subtle))).abs() <= subtle.snap_size as i16
        {
            geom.x = screen.geom.x + (screen.geom.width - geom.width) as i16 - self.get_border_width(subtle);
        }

        // Snap to screen border when value is in snap margin - > Y Axis
        if (screen.geom.y - geom.y).abs() <= subtle.snap_size as i16 {
            geom.y = screen.geom.y + self.get_border_width(subtle);
        } else if ((screen.geom.y + screen.geom.height as i16)
            - (geom.y + geom.height as i16 + self.get_border_width(subtle))).abs() <= subtle.snap_size as i16
        {
             geom.y = screen.geom.y + (screen.geom.height - geom.height) as i16 - self.get_border_width(subtle);
        }

        Ok(())
    }

    /// Warp pointer to center of client
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn warp_pointer(&self, subtle: &Subtle) -> Result<()> {
        ignore_if_dead!(self);

        let conn = subtle.conn.get().unwrap();

        let default_screen = &conn.setup().roots[subtle.screen_num];

        conn.warp_pointer(NONE, default_screen.root, 0, 0, 0, 0,
                          self.geom.x + self.geom.width as i16 / 2,
                          self.geom.y + self.geom.height as i16 / 2)?.check()?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Start dragging of client window
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `drag_mode` - Dragging mode
    /// * `drag_dir` - Dragging direction
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn drag(&mut self, subtle: &Subtle, drag_mode: DragMode, drag_dir: DirectionOrder) -> Result<()> {
        ignore_if_dead!(self);

        let conn = subtle.conn.get().unwrap();
        let query_reply = conn.query_pointer(self.win)?.reply()?;

        let mut geom = Rectangle {
            x: query_reply.root_x - query_reply.win_x,
            y: query_reply.root_y - query_reply.win_y,
            width: self.geom.width,
            height: self.geom.height,
        };

        let screen = subtle.screens.get(self.screen_idx as usize)
            .context("Can't get screen")?;

        // Select starting edge
        let drag_edge = if query_reply.win_x < (geom.width / 2) as i16 {
                DragEdge::LEFT } else { DragEdge::RIGHT }
            | if query_reply.win_y < (geom.height / 2) as i16 {
                DragEdge::TOP } else { DragEdge::BOTTOM };

        // Set variables according to mode
        let cursor = match drag_mode {
            DragMode::MOVE => subtle.move_cursor,
            DragMode::RESIZE => subtle.resize_cursor,
        };

        // Grab pointer and server
        conn.grab_pointer(true, self.win, EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION, GrabMode::ASYNC, GrabMode::ASYNC,
                          NONE, cursor, CURRENT_TIME)?;
        conn.grab_server()?;

        match drag_dir {
            DirectionOrder::Up => {
                if DragMode::RESIZE == drag_mode {
                    geom.y -= self.height_inc as i16;
                    geom.height += self.height_inc;
                } else {
                    geom.y -= subtle.step_size;
                }

                self.snap(subtle, screen, &mut geom)?;
                self.apply_size_hints(subtle, &screen.geom,
                                      false, false, &mut geom);
            },
            DirectionOrder::Right => {
                if DragMode::RESIZE == drag_mode {
                    geom.height += self.height_inc;
                } else {
                    geom.y += subtle.step_size;
                }

                self.snap(subtle, screen, &mut geom)?;
                self.apply_size_hints(subtle, &screen.geom,
                                      false, false, &mut geom);
            },
            DirectionOrder::Down => {
                if DragMode::RESIZE == drag_mode {
                    geom.x -= self.width_inc as i16;
                    geom.width += self.width_inc;
                } else {
                    geom.x -= subtle.step_size;
                }

                self.snap(subtle, screen, &mut geom)?;
                self.apply_size_hints(subtle, &screen.geom,
                                      false, false, &mut geom);
            },
            DirectionOrder::Left => {
                if DragMode::RESIZE == drag_mode {
                    geom.x -= self.width_inc as i16;
                    geom.width += self.width_inc;
                } else {
                    geom.x -= subtle.step_size;
                }

                self.snap(subtle, screen, &mut geom)?;
                self.apply_size_hints(subtle, &screen.geom,
                                      false, false, &mut geom);
            },
            DirectionOrder::Mouse => {
                drag_interactively(subtle, screen, self, &mut geom, &query_reply, drag_mode, drag_edge)?;

                // Subtract border width
                if !self.flags.intersects(ClientFlags::MODE_BORDERLESS) {
                    geom.x -= subtle.clients_style.border.top;
                    geom.y -= subtle.clients_style.border.top;
                }
            }
        }

        self.move_resize(subtle, &geom)?;

        // Remove grabs
        conn.ungrab_pointer(CURRENT_TIME)?;
        conn.ungrab_server()?;

        println!("{}: client={}", function_name!(), self);

        Ok(())
    }


    /// Map client window on display
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn map(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        conn.map_window(self.win)?.check()?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Unmap client window from display
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn unmap(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        conn.unmap_window(self.win)?.check()?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Whether client window is currently visible
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn is_visible(&self, subtle: &Subtle) -> bool {
        subtle.visible_tags.get().intersects(self.tags)
            || self.flags.intersects(ClientFlags::TYPE_DESKTOP | ClientFlags::MODE_STICK)
    }

    /// Whether client is marked as dead (aka ignore further events)
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn is_alive(&self) -> bool {
        !self.flags.intersects(ClientFlags::DEAD)
    }

    /// Convert modes into displayable string
    ///
    /// # Returns
    ///
    /// Mode string
    pub(crate) fn mode_string(&self) -> String {
        let mut mode_str =  String::with_capacity(6);

        // Collect window modes
        if self.flags.intersects(ClientFlags::MODE_FULL) {
            mode_str.push_str("+");
        }
        if self.flags.intersects(ClientFlags::MODE_FLOAT) {
            mode_str.push_str("^");
        }
        if self.flags.intersects(ClientFlags::MODE_STICK) {
            mode_str.push_str("*");
        }
        if self.flags.intersects(ClientFlags::MODE_RESIZE) {
            mode_str.push_str("-");
        }
        if self.flags.intersects(ClientFlags::MODE_ZAPHOD) {
            mode_str.push_str("=");
        }
        if self.flags.intersects(ClientFlags::MODE_FIXED) {
            mode_str.push_str("!");
        }

        mode_str
    }

    /// Send compliant clients the close property and kill the rest
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
        if self.flags.intersects(ClientFlags::CLOSE) {
           ewmh::send_message(subtle, self.win, atoms.WM_PROTOCOLS,
                              &[atoms.WM_DELETE_WINDOW, CURRENT_TIME, 0, 0, 0])?;
        } else {
            let screen_idx = if let Some(focus_client) = subtle.find_focus_client()
                && focus_client.win == self.win { self.screen_idx } else { -1 };

            // Kill it manually
            conn.kill_client(self.win)?.check()?;

            subtle.remove_client_by_win(self.win);

            self.kill(subtle)?;

            publish(subtle, false)?;
        }

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Kill client along with its window
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
        let atoms = subtle.atoms.get().unwrap();

        // Remove _NET_WM_STATE (see EWMH 1.3)
        conn.delete_property(self.win, atoms._NET_WM_STATE)?;

        // Ignore further events
        conn.change_window_attributes(self.win, &ChangeWindowAttributesAux::default()
            .event_mask(EventMask::NO_EVENT))?;

        // Remove client tags from urgent tags
        if self.flags.contains(ClientFlags::MODE_URGENT) {
            subtle.urgent_tags.replace(subtle.urgent_tags.get() - self.tags);
        }

        // Tile remaining clients if necessary
        if self.is_visible(subtle) {
            if let Some(gravity) = subtle.gravities.get(self.gravity_idx as usize) {
               if subtle.flags.contains(SubtleFlags::GRAVITY_TILING)
                   || gravity.flags.contains(GravityFlags::HORZ | GravityFlags::VERT)
               {
                   self.gravity_tile(subtle, self.gravity_idx, self.screen_idx)?;
               }
            }
        }

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Mode and resize client window
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `bounds` - Outer bounds for sizes
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn move_resize(&mut self, subtle: &Subtle, bounds: &Rectangle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Update border and gap
        /*self.geom.x += subtle.clients_style.margin.left;
        self.geom.y += subtle.clients_style.margin.left;
        self.geom.width -= (2 * self.get_border_width(subtle) + subtle.clients_style.margin.left
            + subtle.clients_style.margin.right) as u16;
        self.geom.height -= (2 * self.get_border_width(subtle) + subtle.clients_style.margin.top
            + subtle.clients_style.margin.bottom) as u16;*/

        self.resize(subtle, bounds, true)?;

        let aux = ConfigureWindowAux::default()
            .x(self.geom.x as i32)
            .y(self.geom.y as i32)
            .width(self.geom.width as u32)
            .height(self.geom.height as u32);

        conn.configure_window(self.win, &aux)?.check()?;

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    /// Tile client windows with the same gravity
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `gravity_id` - Gravity index
    /// * `screen_id` - Screen index
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn gravity_tile(&self, subtle: &Subtle, gravity_id: isize, screen_id: isize) -> Result<()> {
        let gravity = subtle.gravities.get(gravity_id as usize)
            .ok_or(anyhow!("Gravity not found"))?;
        let screen = subtle.screens.get(screen_id as usize)
            .ok_or(anyhow!("Screen not found"))?;

        // Pass 1: Count clients with this gravity
        let mut used = 0u16;

        for client in subtle.clients.borrow().iter() {
            if client.gravity_idx == gravity_id && client.screen_idx == screen_id
                && subtle.visible_tags.get().contains(client.tags)
                && !client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL)
            {
                used += 1;
            }
        }

        if 0 == used {
            return Ok(());
        }

        // Calculate tiled gravity value and rounding fix
        let mut geom: Rectangle = Rectangle::default();

        gravity.apply_size(&screen.geom, &mut geom);

        let mut calc = 0;
        let mut round_fix = 0;

        if gravity.flags.contains(GravityFlags::HORZ) {
            calc = geom.width / used;
            round_fix = geom.width - calc * used;
        } else if gravity.flags.contains(GravityFlags::VERT) {
            calc = geom.height / used;
            round_fix = geom.height - calc * used;
        }

        // Pass 2: Update geometry of every client with this gravity
        let mut pos = 0;

        for (client_idx, client) in subtle.clients.borrow().iter().enumerate() {
            if client.gravity_idx == gravity_id && client.screen_idx == screen_id
                && subtle.visible_tags.get().contains(client.tags)
                && !client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL)
            {
                let mut geom = Rectangle::default();

                if gravity.flags.contains(GravityFlags::HORZ) {
                    geom.x = geom.x + (pos * calc) as i16;
                    geom.y = geom.y;
                    geom.width = if pos == used { calc + round_fix } else { calc };
                    geom.height = geom.height;

                    pos += 1;
                } else if gravity.flags.contains(GravityFlags::VERT) {
                    geom.x = geom.x;
                    geom.y = geom.y + (pos * calc) as i16;
                    geom.width = geom.width;
                    geom.height = if pos == used { calc + round_fix } else { calc };

                    pos +=1;
                }

                // Finally update client
                if let Some(mut_client) = subtle.clients.borrow_mut().get_mut(client_idx) {
                    mut_client.geom = geom;

                    mut_client.move_resize(subtle, &screen.geom)?;
                }
            }
        }

        Ok(())
    }

    fn get_border_width(&self, subtle: &Subtle) -> i16 {
        if self.flags.contains(ClientFlags::MODE_BORDERLESS) {
            0
        } else {
            subtle.clients_style.border.top
        }
    }

    fn apply_size_hints(&self, subtle: &Subtle, bounds: &Rectangle,
                        adjust_x: bool, adjust_y: bool, geom: &mut Rectangle)
    {
        if !self.flags.contains(ClientFlags::MODE_FIXED)
            && (self.flags.contains(ClientFlags::MODE_RESIZE)
            || self.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE))
        {
            let border_width = (2 * self.get_border_width(subtle)
                + subtle.clients_style.margin.left
                + subtle.clients_style.margin.right) as u16;

            // Calculate max width and max height for bounds
            let max_width = if -1 == self.max_width {
                bounds.width - border_width } else { self.max_width as u16 };
            let max_height = if -1 == self.max_height {
                bounds.height - border_width } else { self.max_height as u16 };

            // Limit width and height
            if geom.width < self.min_width {
                geom.width = self.min_width;
            }

            if geom.width > max_width {
                geom.width = max_width;
            }

            if geom.height < self.min_height {
               geom.height = self.min_height;
            }

            if geom.height > max_height {
                geom.height = max_height;
            }

            // Adjust based on increment values (see ICCCM 4.1.2.3)
            let diff_width = (geom.width - self.base_width) % self.width_inc;
            let diff_height = (geom.height - self.base_height) % self.height_inc;

            // Adjust x and/or y
            if adjust_x {
                geom.x += diff_width as i16;
            }

            if adjust_y {
                geom.y += diff_height as i16;
            }

            geom.width -= diff_width;
            geom.height -= diff_height;

            // Check aspect ratios
            if 0f32 < self.min_ratio && self.geom.height as f32 * self.min_ratio > self.geom.width as f32 {
                geom.width = (geom.height as f32 * self.min_ratio) as u16;
            }

            if 0f32 < self.max_ratio && self.geom.height as f32 * self.max_ratio < self.geom.width as f32 {
                geom.width = (geom.height as f32 * self.max_ratio) as u16;
            }
        }
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, instance={}, class={}, role={}, win={}, leader={}, \
            geom=(x={}, y={}, width={}, height={}), input={}, focus={}, tags={:?}",
               self.name, self.instance, self.klass, self.role, self.win, self.leader,
               self.geom.x, self.geom.y, self.geom.width, self.geom.height,
               self.flags.contains(ClientFlags::INPUT), self.flags.contains(ClientFlags::FOCUS),
               self.tags)
    }
}

impl Eq for Client {}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.win == other.win
    }
}

impl PartialOrd for Client {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Client {
    fn cmp(&self, other: &Self) -> Ordering {

        // Direction is required when we change stacking on the same level
        let direction = if RestackOrder::Down == self.order {
            Ordering::Less
        } else if RestackOrder::Up == self.order {
            Ordering::Greater
        } else if RestackOrder::Down == other.order {
            Ordering::Greater
        } else if RestackOrder::Up == other.order {
            Ordering::Less
        } else {
            Ordering::Equal
        };

        // Complicated comparison to ensure stacking order.
        // Our desired order is following: Desktop < Gravity < Float < Full
        //
        // This function returns following values:
        //
        // [`Less`] => self is on a lower level
        // [`Equal`] => self and other are on the same level
        // [`Greater`] => self is on a higher level
        //
        if self.flags.intersects(ClientFlags::TYPE_DESKTOP) {
            if other.flags.intersects(ClientFlags::TYPE_DESKTOP) {
                direction
            } else {
                Ordering::Equal
            }
        } else if self.flags.intersects(ClientFlags::MODE_FULL) {
            if other.flags.intersects(ClientFlags::MODE_FULL) {
                direction
            } else {
                Ordering::Greater
            }
        } else if self.flags.intersects(ClientFlags::MODE_FLOAT) {
            if other.flags.intersects(ClientFlags::MODE_FULL) {
                Ordering::Less
            } else if other.flags.intersects(ClientFlags::MODE_FLOAT) {
                direction
            } else {
                Ordering::Greater
            }
        } else {
            if other.flags.intersects(ClientFlags::TYPE_DESKTOP) {
                Ordering::Greater
            } else if other.flags.intersects(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL) {
                Ordering::Less
            } else {
                direction
            }
        }
    }
}

fn draw_mask(subtle: &Subtle, geom: &Rectangle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    conn.poly_rectangle(default_screen.root, subtle.invert_gc, &[Rectangle {
        x: geom.x - 1,
        y: geom.y - 1,
        width: geom.width + 1,
        height: geom.height + 1
    }])?.check()?;

    Ok(())
}

fn drag_interactively(subtle: &Subtle, screen: &Screen, client: &Client, geom: &mut Rectangle,
                      query_reply: &QueryPointerReply, drag_mode: DragMode, drag_edge: DragEdge) -> Result<()>
{
    let conn = subtle.conn.get().unwrap();

    let mut fx = 0;
    let mut fy = 0;
    let mut dx = 0;
    let mut dy = 0;

    // Set starting point
    if drag_edge.intersects(DragEdge::LEFT) {
        fx = geom.x + geom.width as i16;
        dx = query_reply.root_x - client.geom.x;
    } else if drag_edge.intersects(DragEdge::RIGHT) {
        fx = geom.x;
        dx = geom.x + geom.width as i16 - query_reply.root_x;
    }

    if drag_edge.intersects(DragEdge::TOP) {
        fy = geom.y + geom.height as i16;
        dy = query_reply.root_y - client.geom.y;
    } else if drag_edge.intersects(DragEdge::BOTTOM) {
        fy = geom.y;
        dy = geom.y + geom.height as i16 - query_reply.root_y;
    }

    draw_mask(subtle, &geom)?;

    // Start event loop
    'dragging: loop {
        if let Ok(event) = conn.wait_for_event() {
            match event {
                Event::ButtonRelease(evt) => {
                    break 'dragging;
                },
                Event::MotionNotify(evt) => {
                    draw_mask(subtle, &geom)?;

                    if DragMode::MOVE == drag_mode {
                        geom.x = (query_reply.root_x - query_reply.win_x)
                            - (query_reply.root_x - evt.root_x);
                        geom.y = (query_reply.root_y - query_reply.win_y)
                            - (query_reply.root_y - evt.root_y);

                        client.snap(subtle, &screen, geom)?;
                    } else {
                        // Handle resize based on edge
                        if drag_edge.intersects(DragEdge::LEFT) {
                            geom.x = evt.root_x - dx;
                            geom.width = (evt.root_x + dx) as u16;
                        } else if drag_edge.intersects(DragEdge::RIGHT) {
                            geom.x = fx;
                            geom.width = (evt.root_x - fx + dx) as u16;
                        }

                        if drag_edge.intersects(DragEdge::TOP) {
                            geom.y = evt.root_y - dy;
                            geom.height = (fy - evt.root_y + dy) as u16;
                        } else {
                            geom.y = fy;
                            geom.height = (evt.root_y - fy + dy) as u16;
                        }

                        // Adjust bounds based on edge
                        client.apply_size_hints(subtle, &screen.geom,
                                              drag_edge.intersects(DragEdge::LEFT),
                                              drag_edge.intersects(DragEdge::TOP), geom);
                    }

                    draw_mask(subtle, &geom)?;
                },
                _ => {},
            }
        }
    }

    // Erase mask again
    draw_mask(subtle, &geom)?;

    Ok(())
}

fn get_default_gravity(subtle: &Subtle) -> isize {
    let mut grav: isize = subtle.default_gravity;

    // Get gravity from focus client
    if -1 == subtle.default_gravity && let Some(focus) = subtle.find_focus_client() {
        grav = focus.gravity_idx;
    }

    grav
}

fn calc_zaphod(subtle: &Subtle, bounds: &mut Rectangle) -> Result<()> {
    let mut flags = ScreenFlags::TOP_PANEL | ScreenFlags::BOTTOM_PANEL;

    // Update bounds according to styles
    bounds.x = subtle.clients_style.padding.left;
    bounds.y = subtle.clients_style.padding.top;
    bounds.width = subtle.width - (subtle.clients_style.padding.left -
        subtle.clients_style.padding.right) as u16;
    bounds.height = subtle.height - (subtle.clients_style.padding.top -
        subtle.clients_style.padding.bottom) as u16;

    // Iterate over screens to find fitting square
    for screen in subtle.screens.iter() {
        if screen.flags.contains(flags) {
            if screen.flags.contains(ScreenFlags::TOP_PANEL) {
                bounds.y += subtle.panel_height as i16;
                bounds.height -= subtle.panel_height;
            }

            if screen.flags.contains(ScreenFlags::BOTTOM_PANEL) {
                bounds.height -= subtle.panel_height;
            }

            flags &= !(screen.flags & (ScreenFlags::TOP_PANEL | ScreenFlags::BOTTOM_PANEL));
        }
    }

    Ok(())
}

pub(crate) fn find_next(subtle: &'_ Subtle, screen_idx: isize, jump_to_win: bool) -> Option<Ref<'_, Client>> {
    debug!("{}: screen_id={}, jump={}", function_name!(), screen_idx, jump_to_win);

    // Pass 1: Check focus history of current screen
    for win in subtle.focus_history.iter() {
        if let Some(client) = subtle.find_client(*win) {
            if client.screen_idx == screen_idx && client.is_alive() && client.is_visible(subtle)
                && subtle.find_focus_win() != client.win
            {
                return Some(client)
            }
        }
    }

    // Pass 2: Check client stacking list backwards of current screen
    if let Ok(client) = Ref::filter_map(subtle.clients.borrow(), |clients| {
        clients.iter().find(|c| c.screen_idx == screen_idx && c.is_alive() && c.is_visible(subtle))
    }) {
        return Some(client)
    }

    // Pass 3: Check client stacking list backwards of any visible screen
    if 1 < subtle.clients.borrow().len() && jump_to_win {
        if let Ok(client) = Ref::filter_map(subtle.clients.borrow(), |clients| {
            clients.iter().find(|c| c.is_alive() && c.is_visible(subtle) && subtle.find_focus_win() != c.win)
        }) {
            return Some(client)
        }
    }

    None
}

pub(crate) fn restack_clients(order: RestackOrder) -> Result<()> {
    debug!("{}: restack={:?}", function_name!(), order);

    Ok(())
}

/// Publish and export all relevant atoms to allow IPC
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `restack_windows` - Whether to restackl/sort window list
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn publish(subtle: &Subtle, restack_windows: bool) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    let clients = subtle.clients.borrow();
    let mut wins: Vec<u32> = Vec::with_capacity(clients.len());

    // Sort clients from top to bottom
    for (client_idx, client) in clients.iter().enumerate() {
        wins.push(client.win);
    }

    // EWMH: Client list and stacking list (same for us)
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_CLIENT_LIST,
                           AtomEnum::WINDOW, &wins)?;
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_CLIENT_LIST_STACKING,
                           AtomEnum::WINDOW, &wins)?;

    // Restack windows? We assembled the array anyway
    if restack_windows {
        // TODO
        //XRestackWindows
    }

    conn.flush()?;

    debug!("{}: nclients={}, restack={}", function_name!(), clients.len(), restack_windows);

    Ok(())
}
