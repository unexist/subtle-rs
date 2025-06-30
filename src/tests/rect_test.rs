///
/// @package subtle-rs
///
/// @file Rect tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use crate::rect::Rect;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_rect(x: i16, y: i16, width: u16, height: u16) {
        let rect = Rect::from((x, y, width, height));

        assert_eq!(rect.x, x);
        assert_eq!(rect.y, y);
        assert_eq!(rect.width, width);
        assert_eq!(rect.height, height);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_contain_point(x: i16, y: i16, width: u16, height: u16) {
        let rect = Rect::from((x, y, width, height));
        
        assert!(rect.contains_point(x + 5, y + 5));
    }
}