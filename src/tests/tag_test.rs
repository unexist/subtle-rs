///
/// @package subtle-rs
///
/// @file Tag tests
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use crate::tag::TagBuilder;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_tag(s in "[a-zA-Z]*") {
        let mut builder = TagBuilder::default();

        builder.name(s);

        let _ = builder.build().unwrap();
    }
}