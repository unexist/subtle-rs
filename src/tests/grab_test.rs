
///
/// @package subtle-rs
///
/// @file Grab tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use crate::grab::Grab;

#[test]
fn should_parse_keybinding() {
    let grab = Grab::new("subtle_restart", "W-C-S-r");

    assert_eq!(grab.code, 0);
    assert_eq!(grab.state, 0);
}
