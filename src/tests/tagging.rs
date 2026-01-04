///
/// @package subtle-rs
///
/// @file Tagging tests
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use crate::tagging::Tagging;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_add_and_match_tag_from_bits(id in 0u16..31) {
        let other_id = if 31 > id { id + 1 } else { id - 1 };
    
        let tag1 = Tagging::from_bits_retain(1 << id);
        let tag2 = Tagging::from_bits_retain(1 << other_id);
        
        let tagging = tag1;
    
        prop_assert!(tagging.contains(tag1));
        prop_assert!(!tagging.contains(tag2));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_clear_tag_if_set(id1 in 1u16..31, id2 in 1u16..32) {
        let tag1 = Tagging::from_bits_retain(1 << 0);
        let tag2 = Tagging::from_bits_retain(1 << id1);
        let tag3 = Tagging::from_bits_retain(1 << id2);

        let tagging = tag1 | tag3;
        let mut remaining = tagging.clone();

        remaining &= !(tagging & (tag2 | tag3));

        prop_assert!(remaining.contains(tag1));
        prop_assert!(!remaining.contains(tag2));
        prop_assert!(!remaining.contains(tag3));
    }
}