//! Set of traits similar to [`PartialEq`] and [`PartialOrd`] from `std::cmp`
//! which can be implemented on a [`FieldType`] to allow comparing its value with sql.
//!
//! Also contains more traits corresponding to other sql comparison operators.
//!
//! ## Using
//! Don't call the traits' methods directly. Instead use the corresponding method on [`FieldAccess`].
//! Otherwise the assumptions an implementation is allowed to make, might be violated.
//!
//! ## Implementing
//! - Each method takes an [`FieldAccess`]; an implementation may assume that the access' field's type
//!   matches the type the trait is implemented on. This isn't enforced using trait bounds (yet?) to reduce complexity.

use super::FieldType;
use crate::conditions::{Binary, BinaryOperator, Column, Condition};
use crate::internal::field::access::FieldAccess;
use crate::internal::field::{Field, FieldProxy, SingleColumnField};
use crate::internal::relation_path::Path;

/// Trait for equality comparisons.
///
/// **Read module notes, before using.**
pub trait FieldEq<'rhs, Rhs: 'rhs, Any = ()>: FieldType {
    /// Condition type returned from [`FieldEq::field_equals`]
    type EqCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `==`
    fn field_equals<A: FieldAccess>(access: A, value: Rhs) -> Self::EqCond<A>;

    /// Condition type returned from [`FieldEq::field_not_equals`]
    type NeCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `!=`
    fn field_not_equals<A: FieldAccess>(access: A, value: Rhs) -> Self::NeCond<A>;
}

/// Trait for field types that form an order.
///
/// **Read module notes, before using.**
pub trait FieldOrd<'rhs, Rhs: 'rhs, Any = ()>: FieldType {
    /// Condition type returned from [`FieldOrd::field_less_than`]
    type LtCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `<`
    fn field_less_than<A: FieldAccess>(access: A, value: Rhs) -> Self::LtCond<A>;

    /// Condition type returned from [`FieldOrd::field_less_equals`]
    type LeCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `<=`
    fn field_less_equals<A: FieldAccess>(access: A, value: Rhs) -> Self::LeCond<A>;

    /// Condition type returned from [`FieldOrd::field_greater_than`]
    type GtCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `<`
    fn field_greater_than<A: FieldAccess>(access: A, value: Rhs) -> Self::GtCond<A>;

    /// Condition type returned from [`FieldOrd::field_greater_equals`]
    type GeCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `>=`
    fn field_greater_equals<A: FieldAccess>(access: A, value: Rhs) -> Self::GeCond<A>;
}

/// Trait for field types to implement sql's `LIKE` comparison.
///
/// **Read module notes, before using.**
pub trait FieldLike<'rhs, Rhs: 'rhs, Any = ()>: FieldType {
    /// Condition type returned from [`FieldLike::field_like`]
    type LiCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `LIKE`
    fn field_like<A: FieldAccess>(access: A, value: Rhs) -> Self::LiCond<A>;

    /// Condition type returned from [`FieldLike::field_not_like`]
    type NlCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `NOT LIKE`
    fn field_not_like<A: FieldAccess>(access: A, value: Rhs) -> Self::NlCond<A>;
}

/// Trait for field types to implement sql's `REGEXP` comparison.
///
/// **Read module notes, before using.**
pub trait FieldRegexp<'rhs, Rhs: 'rhs, Any = ()>: FieldType {
    /// Condition type returned from [`FieldRegexp::field_regexp`]
    type ReCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `REGEXP`
    fn field_regexp<A: FieldAccess>(access: A, value: Rhs) -> Self::ReCond<A>;

    /// Condition type returned from [`FieldRegexp::field_not_regexp`]
    type NrCond<A: FieldAccess>: Condition<'rhs>;

    /// Compare the field to another value using `NOT REGEXP`
    fn field_not_regexp<A: FieldAccess>(access: A, value: Rhs) -> Self::NrCond<A>;
}

// TODO: null check, BETWEEN, IN

/// Provides the "default" implementation of [`FieldEq`].
///
/// It expects a "usual" impl block
/// whose body is a closure which converts the `Rhs` into a [`Value`]
#[doc(hidden)]
#[allow(non_snake_case)] // makes it clearer that a trait and which trait is meant
#[macro_export]
macro_rules! impl_FieldEq {
    (impl<'rhs $(, $generic:ident $( $const_name:ident : $const_type:ty )?)*> FieldEq<'rhs, $rhs:ty $(, $any:ty)?> for $lhs:ty $(where $( $bound_left:path : $bound_right:path ,)*)? { $into_value:expr }) => {
        impl<'rhs $(, $generic $($const_name : $const_type)?)*> $crate::fields::traits::cmp::FieldEq<'rhs, $rhs $(, $any)?> for $lhs
        where
            $lhs: $crate::fields::traits::FieldType,
            $($( $bound_left : $bound_right ,)*)?
        {
            type EqCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_equals<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::EqCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::Equals,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }

            type NeCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_not_equals<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::NeCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::NotEquals,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }
        }
    };
}

// Impl FieldEq<FieldProxy> iff FieldEq<Self>
impl<'rhs, F, P, T> FieldEq<'rhs, FieldProxy<F, P>> for T
where
    T: FieldEq<'rhs, T>,
    F: Field<Type = T> + SingleColumnField,
    P: Path,
{
    type EqCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;

    fn field_equals<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::EqCond<A> {
        Binary {
            operator: BinaryOperator::Equals,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }

    type NeCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;

    fn field_not_equals<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::NeCond<A> {
        Binary {
            operator: BinaryOperator::NotEquals,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }
}

/// Provides the "default" implementation of [`FieldOrd`].
///
/// It takes
/// - the left hand side type i.e. type to implement on
/// - the right hand side (use `'rhs` a lifetime if required)
/// - a closure to convert the right hand side into a [`Value`]
#[doc(hidden)]
#[allow(non_snake_case)] // makes it clearer that a trait and which trait is meant
#[macro_export]
macro_rules! impl_FieldOrd {
    ($lhs:ty, $rhs:ty, $into_value:expr) => {
        impl<'rhs> $crate::fields::traits::cmp::FieldOrd<'rhs, $rhs> for $lhs {
            type LtCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_less_than<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::LtCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::Less,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }

            type LeCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_less_equals<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::LeCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::LessOrEquals,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }

            type GtCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_greater_than<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::GtCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::Greater,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }

            type GeCond<A: $crate::FieldAccess> = $crate::conditions::Binary<$crate::conditions::Column<A>, $crate::conditions::Value<'rhs>>;
            fn field_greater_equals<A: $crate::FieldAccess>(access: A, value: $rhs) -> Self::GeCond<A> {
                $crate::conditions::Binary {
                    operator: $crate::conditions::BinaryOperator::GreaterOrEquals,
                    fst_arg: $crate::conditions::Column(access),
                    #[allow(clippy::redundant_closure_call)] // clean way to pass code to a macro
                    snd_arg: $into_value(value),
                }
            }
        }
    };
}

// Impl FieldOrd<FieldProxy> iff FieldOrd<Self>
impl<'rhs, F, P, T> FieldOrd<'rhs, FieldProxy<F, P>> for T
where
    T: FieldOrd<'rhs, T>,
    F: Field<Type = T> + SingleColumnField,
    P: Path,
{
    type LtCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;
    fn field_less_than<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::LtCond<A> {
        Binary {
            operator: BinaryOperator::Less,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }

    type LeCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;
    fn field_less_equals<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::LeCond<A> {
        Binary {
            operator: BinaryOperator::LessOrEquals,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }

    type GtCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;
    fn field_greater_than<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::GtCond<A> {
        Binary {
            operator: BinaryOperator::Greater,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }

    type GeCond<A: FieldAccess> = Binary<Column<A>, Column<FieldProxy<F, P>>>;
    fn field_greater_equals<A: FieldAccess>(access: A, value: FieldProxy<F, P>) -> Self::GeCond<A> {
        Binary {
            operator: BinaryOperator::GreaterOrEquals,
            fst_arg: Column(access),
            snd_arg: Column(value),
        }
    }
}
