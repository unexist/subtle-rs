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

use proptest::prelude::*;
use std::collections::HashMap;
use x11rb::protocol::xproto::{Keycode, Keysym, ModMask};
use crate::grab;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    #[test]
    fn should_parse_key_combinations(key in "([WCS]-){1,3}[a-z]") {
        let mut mapping: HashMap<Keysym, Keycode> = HashMap::new();

        mapping.insert(x11_keysymdef::lookup_by_name(
            &key.chars().last().unwrap().to_string()).unwrap().keysym, key.chars().last().unwrap() as u8);

        if let Ok((_keycode, state, _is_mouse)) = grab::parse_keys(&*key, &mapping) {
            prop_assert!(ModMask::ANY != state);
        } else {
            prop_assert!(false);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_parse_mouse(key in "([WCS]-){1,3}B[1-9]") {
        let mut mapping: HashMap<Keysym, Keycode> = HashMap::new();

        mapping.insert(x11_keysymdef::lookup_by_name(
            &key.chars().last().unwrap().to_string()).unwrap().keysym, key.chars().last().unwrap() as u8);

        if let Ok((keycode, state, is_mouse)) = grab::parse_keys(&*key, &mapping) {
            prop_assert!(0 < keycode);
            prop_assert!(ModMask::ANY != state);
            prop_assert!(is_mouse);
        } else {
            panic!();
        }
    }
}
