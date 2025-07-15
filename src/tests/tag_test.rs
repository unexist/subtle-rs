///
/// @package subtle-rs
///
/// @file Tag tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use crate::tag::Tag;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_tag(s in "[a-zA-Z]*") {
        match Tag::new(&*s) {
            Ok(tag) => assert!(!tag.name.is_empty()),
            Err(err) => {
                if !s.is_empty() {
                    panic!("Expected valid tag");
                }
            }
        }
    }
}