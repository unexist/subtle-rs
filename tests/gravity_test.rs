///
/// @package subtle-rs
///
/// @file Gravity tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

extern crate subtle_rs;

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_stay_in_bounds(x in 1u16..999, y in 1u16..999,
        width in 1u16..999, height in 1u16..999)
    {
        let grav = Gravity::new("test".into(), x, y, width, height);

        assert!(0 <= grav.geom.x && 100 >= grav.geom.x);
        assert!(0 <= grav.geom.y && 100 >= grav.geom.y);
        assert!(0 <= grav.geom.width && 100 >= grav.geom.width);
        assert!(0 <= grav.geom.height && 100 >= grav.geom.height);
    }
}