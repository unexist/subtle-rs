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
use crate::grab;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    #[test]
    fn should_parse_key_combinations(key in "([WCS]-){1,3}[a-z]") {
        if let Ok((_sym, _code, state, _is_mouse)) = grab::parse_keys(&*key) {
            prop_assert!(0 < state);
        } else {
            prop_assert!(false);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_parse_mouse(key in "([WCS]-){1,3}B[1-9]") {
        if let Ok((sym, code, state, is_mouse)) = grab::parse_keys(&*key) {
            prop_assert!(0 < sym);
            prop_assert!(0 < code);
            prop_assert!(0 < state);
            prop_assert!(is_mouse);
        } else {
            panic!();
        }
    }
}
