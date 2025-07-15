///
/// @package subtle-rs
///
/// @file View tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use x11rb::protocol::xproto::Rectangle;
use crate::style::{CalcSide, Side, Style};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_calculate_side(n in 0i16..100) {
        let side = Side {
            top: n,
            right: n,
            bottom: n * 2,
            left: n * 2,
        };

        let style = Style {
            border: side,
            padding: side,
            margin: side,
            ..Default::default()
        };

        assert_eq!(style.calc_side(CalcSide::Top), n * 3);
        assert_eq!(style.calc_side(CalcSide::Right), n * 3);
        assert_eq!(style.calc_side(CalcSide::Bottom), n * 2 * 3);
        assert_eq!(style.calc_side(CalcSide::Left), n * 2 * 3);
    }
}