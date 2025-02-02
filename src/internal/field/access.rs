//! Experimental trait to hide a [`FieldProxy`]s two generics behind a single one.

use std::marker::PhantomData;

use rorm_db::sql::aggregation::SelectAggregator;

use crate::conditions::{Binary, Column, In, InOperator, Value};
use crate::crud::selector::AggregatedColumn;
use crate::fields::traits::{
    FieldAvg, FieldCount, FieldEq, FieldLike, FieldMax, FieldMin, FieldOrd, FieldRegexp, FieldSum,
};
use crate::internal::field::{Field, FieldProxy};
use crate::internal::relation_path::Path;

#[allow(non_snake_case)] // the macro produces a datatype which are named using CamelCase
macro_rules! FieldType {
    () => {
        <Self::Field as Field>::Type
    };
}

/// Trait only implemented by [`FieldProxy`] to reduce the amount of generics when using them.
///
/// ## Why
/// ```no_run
/// # use rorm::internal::field::{FieldProxy, Field, access::FieldAccess};
/// # use rorm::internal::relation_path::Path;
///
/// // function using FieldProxy
/// fn do_something<F, P>(proxy: FieldProxy<F, P>) {/* ... */}
///
/// // but in order to do useful things with the proxy, you will need bounds:
/// fn do_useful<F: Field, P: Path>(proxy: FieldProxy<F, P>) {/* ... */}
///
/// // function using FieldAccess
/// fn do_something_else<A: FieldAccess>(proxy: A) {/* ... */}
///
/// // the above already covers the useful part, but depending on your usage you could also use the `impl` sugar:
/// fn do_sugared(proxy: impl FieldAccess) {/* ... */}
/// ```
///
/// ## Comparison operations
/// This trait also adds methods for comparing fields which just wrap their underlying [comparison traits](crate::fields::traits).
/// ```no_run
/// use rorm::Model;
/// use rorm::internal::field::access::FieldAccess;
///
/// #[derive(Model)]
/// struct User {
///     #[rorm(id)]
///     id: i64,
///
///     #[rorm(max_length = 255)]
///     name: String,
/// }
///
/// // Uses the `FieldEq` impl of `String`
/// let condition = User.name.equals("Bob".to_string());
/// ```
pub trait FieldAccess: Copy + Sized + Send + Sync + 'static {
    /// Field which is accessed
    ///
    /// Corresponds to the proxy's `F` parameter
    type Field: Field;

    /// Path the field is accessed through
    ///
    /// Corresponds to the proxy's `P` parameter
    type Path: Path;

    /// Compare the field to another value using `==`
    fn equals<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldEq<'rhs, Rhs, Any>>::EqCond<Self>
    where
        FieldType!(): FieldEq<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_equals(self, rhs)
    }

    /// Compare the field to another value using `!=`
    fn not_equals<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldEq<'rhs, Rhs, Any>>::NeCond<Self>
    where
        FieldType!(): FieldEq<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_not_equals(self, rhs)
    }

    /// Check if the field's value is in a given list of values
    fn r#in<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: impl IntoIterator<Item = Rhs>,
    ) -> In<Column<Self>, Value<'rhs>>
    where
        FieldType!(): FieldEq<'rhs, Rhs, Any, EqCond<Self> = Binary<Column<Self>, Value<'rhs>>>,
    {
        let values = rhs
            .into_iter()
            .map(|rhs| self.equals(rhs).snd_arg)
            .collect();
        In {
            operator: InOperator::In,
            fst_arg: Column(self),
            snd_arg: values,
        }
    }

    /// Check if the field's value is not in a given list of values
    fn not_in<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: impl IntoIterator<Item = Rhs>,
    ) -> In<Column<Self>, Value<'rhs>>
    where
        FieldType!(): FieldEq<'rhs, Rhs, Any, EqCond<Self> = Binary<Column<Self>, Value<'rhs>>>,
    {
        let values = rhs
            .into_iter()
            .map(|rhs| self.equals(rhs).snd_arg)
            .collect();
        In {
            operator: InOperator::NotIn,
            fst_arg: Column(self),
            snd_arg: values,
        }
    }

    /// Compare the field to another value using `<`
    fn less_than<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldOrd<'rhs, Rhs, Any>>::LtCond<Self>
    where
        FieldType!(): FieldOrd<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_less_than(self, rhs)
    }

    /// Compare the field to another value using `<=`
    fn less_equals<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldOrd<'rhs, Rhs, Any>>::LeCond<Self>
    where
        FieldType!(): FieldOrd<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_less_equals(self, rhs)
    }

    /// Compare the field to another value using `<`
    fn greater_than<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldOrd<'rhs, Rhs, Any>>::GtCond<Self>
    where
        FieldType!(): FieldOrd<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_greater_than(self, rhs)
    }

    /// Compare the field to another value using `>=`
    fn greater_equals<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldOrd<'rhs, Rhs, Any>>::GeCond<Self>
    where
        FieldType!(): FieldOrd<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_greater_equals(self, rhs)
    }

    /// Compare the field to another value using `LIKE`
    fn like<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldLike<'rhs, Rhs, Any>>::LiCond<Self>
    where
        FieldType!(): FieldLike<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_like(self, rhs)
    }

    /// Compare the field to another value using `NOT LIKE`
    fn not_like<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldLike<'rhs, Rhs, Any>>::NlCond<Self>
    where
        FieldType!(): FieldLike<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_not_like(self, rhs)
    }

    /// Compare the field to another value using `>=`
    fn regexp<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldRegexp<'rhs, Rhs, Any>>::ReCond<Self>
    where
        FieldType!(): FieldRegexp<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_regexp(self, rhs)
    }

    /// Compare the field to another value using `>=`
    fn not_regexp<'rhs, Rhs: 'rhs, Any>(
        self,
        rhs: Rhs,
    ) -> <FieldType!() as FieldRegexp<'rhs, Rhs, Any>>::NrCond<Self>
    where
        FieldType!(): FieldRegexp<'rhs, Rhs, Any>,
    {
        <FieldType!()>::field_not_regexp(self, rhs)
    }

    /// Returns the count of the number of times that the column is not null.
    fn count(self) -> AggregatedColumn<Self, i64>
    where
        FieldType!(): FieldCount,
    {
        AggregatedColumn {
            sql: SelectAggregator::Count,
            alias: "count",
            field_access: PhantomData,
            result: PhantomData,
        }
    }

    /// Returns the summary off all non-null values in the group.
    /// If there are only null values in the group, this function will return null.
    fn sum(self) -> AggregatedColumn<Self, <FieldType!() as FieldSum>::Result>
    where
        FieldType!(): FieldSum,
    {
        AggregatedColumn {
            sql: SelectAggregator::Sum,
            alias: "sum",
            field_access: PhantomData,
            result: PhantomData,
        }
    }

    /// Returns the average value of all non-null values.
    /// The result of avg is a floating point value, except all input values are null, than the
    /// result will also be null.
    fn avg(self) -> AggregatedColumn<Self, Option<f64>>
    where
        FieldType!(): FieldAvg,
    {
        AggregatedColumn {
            sql: SelectAggregator::Avg,
            alias: "avg",
            field_access: PhantomData,
            result: PhantomData,
        }
    }

    /// Returns the maximum value of all values in the group.
    /// If there are only null values in the group, this function will return null.
    fn max(self) -> AggregatedColumn<Self, <FieldType!() as FieldMax>::Result>
    where
        FieldType!(): FieldMax,
    {
        AggregatedColumn {
            sql: SelectAggregator::Max,
            alias: "max",
            field_access: PhantomData,
            result: PhantomData,
        }
    }

    /// Returns the minimum value of all values in the group.
    /// If there are only null values in the group, this function will return null.
    fn min(self) -> AggregatedColumn<Self, <FieldType!() as FieldMin>::Result>
    where
        FieldType!(): FieldMin,
    {
        AggregatedColumn {
            sql: SelectAggregator::Min,
            alias: "min",
            field_access: PhantomData,
            result: PhantomData,
        }
    }
}

impl<F: Field, P: Path> FieldAccess for FieldProxy<F, P> {
    type Field = F;
    type Path = P;
}
