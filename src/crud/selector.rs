//! Trait for selecting stuff

use std::marker::PhantomData;

use rorm_db::row::DecodeOwned;
use rorm_db::sql::aggregation::SelectAggregator;

use crate::crud::decoder::{Decoder, DirectDecoder};
use crate::fields::proxy::{FieldProxy, FieldProxyImpl};
use crate::fields::traits::FieldType;
use crate::internal::field::decoder::FieldDecoder;
use crate::internal::field::Field;
use crate::internal::query_context::QueryContext;
use crate::internal::relation_path::{Path, PathField};
use crate::model::{Model, PatchSelector};
use crate::Patch;

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

/// Combinator which wraps a selector to apply a path to it.
pub struct PathedSelector<S, P> {
    selector: S,
    path: PhantomData<P>,
}

impl<S, P> Selector for PathedSelector<S, P>
where
    S: Selector,
    P: Path<Current = S::Model>,
{
    type Result = S::Result;
    type Model = P::Origin;
    type Decoder = S::Decoder;
    const INSERT_COMPATIBLE: bool = P::IS_ORIGIN && S::INSERT_COMPATIBLE;

    fn select(self, ctx: &mut QueryContext) -> Self::Decoder {
        let mut ctx = ctx.with_base_path::<P>();
        self.selector.select(&mut *ctx)
    }
}

impl<T, F, P, I> Selector for FieldProxy<I>
where
    T: FieldType,
    F: Field<Type = T>,
    P: Path,
    I: FieldProxyImpl<Field = F, Path = P>,
{
    type Result = F::Type;
    type Model = P::Origin;
    type Decoder = T::Decoder;
    const INSERT_COMPATIBLE: bool = P::IS_ORIGIN;

    fn select(self, ctx: &mut QueryContext) -> Self::Decoder {
        FieldDecoder::new(ctx, self)
    }
}

#[doc(hidden)]
impl<F, P, I> FieldProxy<I>
where
    F: Field + PathField<<F as Field>::Type>,
    P: Path<Current = <F::ParentField as Field>::Model>,
    I: FieldProxyImpl<Field = F, Path = P>,
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
pub struct AggregatedColumn<I, R> {
    pub(crate) sql: SelectAggregator,
    pub(crate) alias: &'static str,
    pub(crate) field: FieldProxy<I>,
    pub(crate) result: PhantomData<R>,
}
impl<I, R> Selector for AggregatedColumn<I, R>
where
    I: FieldProxyImpl,
    R: DecodeOwned,
{
    type Result = R;
    type Model = <I::Path as Path>::Origin;
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
