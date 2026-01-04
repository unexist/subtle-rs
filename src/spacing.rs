///
/// @package subtle-rs
///
/// @file Spacing functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use anyhow::anyhow;
use crate::config::MixedConfigVal;

#[derive(Default, Debug, PartialEq, Copy, Clone)]
pub(crate) struct Spacing {
    pub(crate) top: i16,
    pub(crate) right: i16,
    pub(crate) bottom: i16,
    pub(crate) left: i16,
}

impl Spacing {
    pub(crate) fn inherit(&mut self, other_space: &Spacing, merge: bool) {
        // Inherit unset values
        if -1 == self.top || (merge && -1 != other_space.top) {
            self.top = other_space.top;
        }

        if -1 == self.right || (merge && -1 != other_space.right) {
            self.right = other_space.right;
        }

        if -1 == self.bottom || (merge && -1 != other_space.bottom) {
            self.bottom = other_space.bottom;
        }

        if -1 == self.left || (merge && -1 != other_space.left) {
            self.left = other_space.left;
        }
    }

    pub(crate) fn reset(&mut self, default_value: i16) {
        // Set values
        self.top = default_value;
        self.right = default_value;
        self.bottom = default_value;
        self.left = default_value;
    }
}

impl fmt::Display for Spacing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(top={}, right={}, bottom={}, left={})",
               self.top, self.right, self.bottom, self.left)
    }
}

impl TryFrom<&MixedConfigVal> for Spacing {
    type Error = anyhow::Error;

    fn try_from(value: &MixedConfigVal) -> Result<Self, Self::Error> {
        match value {
            MixedConfigVal::I(val) => Ok(Self {
                top: *val as i16,
                right: *val as i16,
                left: *val as i16,
                bottom: *val as i16,
            }),
            MixedConfigVal::VI(val) => {
                match val.len() {
                    2 => Ok(Self {
                        top: val[0] as i16,
                        right: val[1] as i16,
                        left: val[1] as i16,
                        bottom: val[0] as i16,
                    }),
                    3 => Ok(Self {
                        top: val[0] as i16,
                        right: val[1] as i16,
                        left: val[1] as i16,
                        bottom: val[2] as i16,
                    }),
                    4 => Ok(Self {
                        top: val[0] as i16,
                        right: val[1] as i16,
                        left: val[2] as i16,
                        bottom: val[3] as i16,
                    }),
                    _ => Err(anyhow!("Too many values for spacing")),
                }
            }
            _ => Err(anyhow!("Invalid type for spacing")),
        }
    }
}
