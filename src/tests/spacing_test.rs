///
/// @package subtle-rs
///
/// @file Spacing tests
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use proptest::prelude::*;
use proptest::collection::{vec, VecStrategy};
use crate::config::MixedConfigVal;
use crate::spacing::Spacing;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_from_integer(n in 0i16..100) {
        let spacing = Spacing::try_from(&MixedConfigVal::I(n as i32));

        prop_assert!(spacing.is_ok());
        prop_assert_eq!(spacing.unwrap(), Spacing {
            top: n,
            right: n,
            bottom: n,
            left: n,
        });
    }
}

fn vec_strategy(count: usize) -> VecStrategy<proptest::num::i32::Any> {
    vec(any::<i32>(), 0..count)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_from_vector_n2(v in vec_strategy(2)) {
        let spacing = Spacing::try_from(&MixedConfigVal::VI(v.clone()));

        prop_assert!(spacing.is_ok());
        prop_assert_eq!(spacing.unwrap(), Spacing {
            top: v[0] as i16,
            right: v[1] as i16,
            bottom: v[1] as i16,
            left: v[0] as i16,
        });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_from_vector_n3(v in vec_strategy(3)) {
        let spacing = Spacing::try_from(&MixedConfigVal::VI(v.clone()));

        prop_assert!(spacing.is_ok());
        prop_assert_eq!(spacing.unwrap(), Spacing {
            top: v[0] as i16,
            right: v[1] as i16,
            bottom: v[1] as i16,
            left: v[2] as i16,
        });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn should_create_from_vector_n4(v in vec_strategy(4)) {
        let spacing = Spacing::try_from(&MixedConfigVal::VI(v.clone()));

        prop_assert!(spacing.is_ok());
        prop_assert_eq!(spacing.unwrap(), Spacing {
            top: v[0] as i16,
            right: v[1] as i16,
            bottom: v[2] as i16,
            left: v[3] as i16,
        });
    }
}