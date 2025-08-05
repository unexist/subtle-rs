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
use crate::view::ViewBuilder;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_view(s in "[a-zA-Z]*") {
        let mut builder = ViewBuilder::default();
        
        builder.name(s);

        let _ = builder.build().unwrap();
    }
}