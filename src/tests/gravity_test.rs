///
/// @package subtle-rs
///
/// @file Gravity tests
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use x11rb::protocol::xproto::Rectangle;
use crate::gravity::Gravity;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    #[allow(unused_comparisons)]
    fn should_stay_in_bounds(x in 0u16..999, y in 0u16..999,
        width in 1u16..999, height in 1u16..999)
    {
        let gravity = Gravity::new("test".into(), x, y, width, height);
        
        prop_assert!(0 <= gravity.geom.x && 100 >= gravity.geom.x);
        prop_assert!(0 <= gravity.geom.y && 100 >= gravity.geom.y);
        prop_assert!(0 <= gravity.geom.width && 100 >= gravity.geom.width);
        prop_assert!(0 <= gravity.geom.height && 100 >= gravity.geom.height);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_calcluate_geom(x in 0i16..999, y in 0i16..999,
        width in 1u16..999, height in 1u16..999)
    {
        let gravity = Gravity::new("test".into(), 0, 0, 50, 50);

        let mut geom = Rectangle::default();
        let bounds = Rectangle {
            x,
            y,
            width,
            height
        };

        gravity.apply_size(&bounds, &mut geom);

        prop_assert_eq!(geom.x, geom.x);
        prop_assert_eq!(geom.y, geom.y);
        prop_assert_eq!(geom.width, width * 50 / 100);
        prop_assert_eq!(geom.height, height * 50 / 100);
    }
}