///
/// @package subtle-rs
///
/// @file Tagging tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::ProptestConfig;
use proptest::proptest;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_add_and_match_tag(id in 1u16..30) 
        let taggings = Taggings::empty();
    
        let tag = Taggings::from_bits_retain(1 << id);
    
        assert!(view.is_ok());
    }
}