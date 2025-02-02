//! Trait for selecting stuff

use std::marker::PhantomData;

use rorm_db::row::DecodeOwned;
use rorm_db::sql::aggregation::SelectAggregator;

use crate::crud::decoder::{Decoder, DirectDecoder};
use crate::fields::traits::FieldType;
use crate::internal::field::decoder::FieldDecoder;
use crate::internal::field::{Field, FieldProxy};
use crate::internal::query_context::QueryContext;
use crate::internal::relation_path::{Path, PathField};
use crate::model::{Model, PatchSelector};
use crate::{FieldAccess, Patch};

/// Something which "selects" a value from a certain table,
/// by configuring a [`QueryContext`] and providing a [`Decoder`]
pub trait Selector {
    /// The value selected by this selector
    type Result;

    /// [`Model`] from whose table to select from
    type Model: Model;

    /// [`Decoder`] to decode the selected value from a [`&Row`](rorm_db::Row)
    type Decoder: Decoder<Result = Self::Result>;

    /// Can this selector be used in insert queries to specify the returning expression?
    const INSERT_COMPATIBLE: bool;

    /// Constructs a decoder and configures a [`QueryContext`] to query the required columns
    fn select(self, ctx: &mut QueryContext) -> Self::Decoder;
}

impl<F, P> Selector for FieldProxy<F, P>
where
    P: Path,
    F: Field,
{
    type Result = F::Type;
    type Model = P::Origin;
    type Decoder = <F::Type as FieldType>::Decoder;
    const INSERT_COMPATIBLE: bool = P::IS_ORIGIN;

    fn select(self, ctx: &mut QueryContext) -> Self::Decoder {
        FieldDecoder::new(ctx, FieldProxy::<F, P>::new())
    }
}

#[doc(hidden)]
impl<F, P> FieldProxy<F, P>
where
    F: Field + PathField<<F as Field>::Type>,
    P: Path<Current = <F::ParentField as Field>::Model>,
{
    pub fn select_as<Ptch>(self) -> PatchSelector<Ptch, P::Step<F>>
    where
        Ptch: Patch<Model = <F::ChildField as Field>::Model>,
    {
        PatchSelector::new()
    }
}

/// A column to select and call an aggregation function on
#[derive(Copy, Clone)]
pub struct AggregatedColumn<A, R> {
    pub(crate) sql: SelectAggregator,
    pub(crate) alias: &'static str,
    pub(crate) field_access: PhantomData<A>,
    pub(crate) result: PhantomData<R>,
}
impl<A, R> Selector for AggregatedColumn<A, R>
where
    A: FieldAccess,
    R: DecodeOwned,
{
    type Result = R;
    type Model = <A::Path as Path>::Origin;
    type Decoder = DirectDecoder<R>;
    const INSERT_COMPATIBLE: bool = false;

    fn select(self, ctx: &mut QueryContext) -> Self::Decoder {
        let (index, column) = ctx.select_aggregation(self);
        DirectDecoder {
            result: PhantomData,
            column,
            index,
        }
    }
}

macro_rules! selectable {
    ($($index:tt : $S:ident,)+) => {
        impl<M: Model, $($S: Selector<Model = M>),+> Selector for ($($S,)+)
        {
            type Result = ($(
                $S::Result,
            )+);

            type Model = M;

            type Decoder = ($(
                $S::Decoder,
            )+);

            const INSERT_COMPATIBLE: bool = $($S::INSERT_COMPATIBLE &&)+ true;

            fn select(self, ctx: &mut QueryContext) -> Self::Decoder {
                ($(
                    self.$index.select(ctx),
                )+)
            }
        }
    };
}
rorm_macro::impl_tuple!(selectable, 1..33);
