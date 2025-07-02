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

use proptest::prelude::*;
use crate::tagging::Tagging;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_add_and_match_tag(id1 in 0u16..31, id2 in 0u16..32) {
        let mut tagging = Tagging::empty();
    
        let tag1 = Tagging::from_bits_retain(1 << id1);
        let tag2 = Tagging::from_bits_retain(1 << id2);
        
        tagging.insert(tag1);
    
        assert!(tagging.contains(tag1));
        
        if id1 != id2 {
            assert!(!tagging.contains(tag2));
        }
    }
}