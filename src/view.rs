///
/// @package subtle-rs
///
/// @file View functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use std::cell::Cell;
use bitflags::bitflags;
use regex::{Regex, RegexBuilder};
use anyhow::Result;
use derive_builder::Builder;
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::protocol::xproto::{AtomEnum, PropMode, Window};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::config::{Config, MixedConfigVal};
use crate::{client};
use crate::subtle::Subtle;
use crate::tagging::Tagging;
use crate::icon::Icon;

bitflags! {
    /// Config and state-flags for [`View`]
    #[derive(Default, Debug, Clone)]
    pub(crate) struct ViewFlags: u32 {
        /// View icon
        const MODE_ICON = 1 << 0;
        /// Icon only
        const MODE_ICON_ONLY = 1 << 1;
        /// Dynamic views
        const MODE_DYNAMIC = 1 << 2;
        /// Stick view
        const MODE_STICK = 1 << 3;
    }
}

#[derive(Default, Builder)]
#[builder(default)]
pub(crate) struct View {
    /// Config and state-flags
    pub(crate) flags: ViewFlags,
    /// Current tagging
    pub(crate) tags: Tagging,
    /// Name of this view
    pub(crate) name: String,
    /// Regex to match tags
    pub(crate) regex: Option<Regex>,
    /// Current win with focus
    pub(crate) focus_win: Cell<Window>,
    /// View icon if any
    pub(crate) icon: Option<Icon>,
}

impl View {
    /// Re-apply all tags to this view
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
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

    /// Focus view and swap views if more than one screen is active
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `screen_idx` - Index of the screens vector
    /// * `swap_views` - Whether views shall be swapped
    /// * `focus_next` - Focus first visible client on view switch
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn focus(&self, subtle: &Subtle, screen_idx: usize, swap_views: bool, focus_next: bool) -> Result<()> {
        if let Some(screen) = subtle.screens.get(screen_idx) {
            if let Some(view_idx) = subtle.views.iter().position(|v| v == self) {

                // Check if view is visible on any screen
                if subtle.visible_views.get().intersects(Tagging::from_bits_retain(1 << (view_idx + 1))) {

                    // This only makes sense with more than one screen - ignore otherwise
                    if 1 < subtle.screens.len() {

                        // Find screen with view and swap
                        for other_screen in subtle.screens.iter() {
                            if other_screen.view_idx.get() == view_idx as isize {
                                if swap_views {
                                    other_screen.view_idx.set(screen.view_idx.get());
                                    screen.view_idx.set(view_idx as isize);
                                } else {
                                    //screen.warp();
                                }

                                break;
                            }
                        }
                    }
                } else {
                    screen.view_idx.set(view_idx as isize);
                }
            }
        }

        if focus_next {
            // Restore focus on view
            if let Some(focus_client) = subtle.find_client(self.focus_win.get()) {
                if focus_client.is_visible(subtle) {
                    focus_client.focus(subtle, true)?;
                } else {
                    self.focus_win.set(NONE);
                }
            } else if let Some(focus_client) = client::find_next(subtle,
                                                                 screen_idx as isize, false)
            {
                focus_client.focus(subtle, true)?;
            }
        }

        debug!("{}: {}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(name={}, regex={:?}, tags={:?})", self.name, self.regex, self.tags)
    }
}

impl PartialEq for View {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

/// Check config and init all view related options
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
    for values in config.views.iter() {
        let mut flags = ViewFlags::empty();
        let mut builder = ViewBuilder::default();

        if let Some(MixedConfigVal::S(name)) = values.get("name") {
            builder.name(name.into());
        }

        if let Some(MixedConfigVal::S(value)) = values.get("match") {
            builder.regex(Some(RegexBuilder::new(value)
                .case_insensitive(true)
                .build()?));
        }

        if let Some(MixedConfigVal::B(icon_only)) = values.get("icon_only") && *icon_only {
            flags.insert(ViewFlags::MODE_ICON_ONLY);
        }

        if let Some(MixedConfigVal::S(icon_file)) = values.get("icon") {
            if let Ok(icon) = Icon::new(subtle, icon_file) {
                flags.insert(ViewFlags::MODE_ICON);
                builder.icon(Some(icon));
            }
        }

        // Finally create view and apply tagging
        builder.flags(flags);

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
