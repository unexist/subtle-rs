///
/// @package subtle-rs
///
/// @file Style tests
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use crate::spacing::Spacing;
use crate::style::{CalcSpacing, Style};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_calculate_spacings(n in 0i16..100) {
        let spacing = Spacing {
            top: n,
            right: n,
            bottom: n * 2,
            left: n * 2,
        };

        let style = Style {
            border: spacing,
            padding: spacing,
            margin: spacing,
            ..Default::default()
        };

        prop_assert_eq!(style.calc_spacing(CalcSpacing::Top), n * 3);
        prop_assert_eq!(style.calc_spacing(CalcSpacing::Right), n * 3);
        prop_assert_eq!(style.calc_spacing(CalcSpacing::Bottom), n * 2 * 3);
        prop_assert_eq!(style.calc_spacing(CalcSpacing::Left), n * 2 * 3);
    }
}
