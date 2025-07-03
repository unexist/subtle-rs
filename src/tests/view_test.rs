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
use crate::view::View;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_view(s in "[a-zA-Z]*") {
        let view = View::new(&*s);

        assert!(!view.name.is_empty());
    }
}