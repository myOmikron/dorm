//! [`Decoder`] trait and some basic implementations

use std::marker::PhantomData;

use rorm_db::row::{DecodeOwned, RowError};
use rorm_db::Row;

/// Something which decodes a [value](Self::Result) from a [`&Row`](rorm_db::Row)
///
/// It is basically a closure `Fn(&Row) -> Result<Self::Result, Error>`.
/// Sadly we need to support decoding via indexes so this trait actually has two method.
/// One for decoding [`by_name`](Self::by_name) and another one for decoding [`by_index`](Self::by_index).
///
/// This trait does not manage
/// a) how the decoder is constructed
/// and b) that the row contains the columns which the decoder will access
///
/// These concerns are delegated to further sub-traits, namely:
/// - [`Selector`](super::selector::Selector) which constructs a [`Decoder`] and configures the [`QueryContext`](crate::internal::query_context::QueryContext) appropriately
/// - [`FieldDecoder`](crate::internal::field::decoder::FieldDecoder) which decodes and is associated to single fields through [`FieldType::Decoder`](crate::fields::traits::FieldType::Decoder)
pub trait Decoder {
    /// The value decoded from a row
    type Result;

    /// Decode a value from a row using select aliases to access the columns
    fn by_name<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>>;

    /// Decode a value from a row using indexes to access the columns
    fn by_index<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>>;
}

/// A [`Decoder`] which directly decodes a [`T: DecodedOwned`](DecodeOwned)
pub struct DirectDecoder<T> {
    pub(crate) result: PhantomData<T>,
    pub(crate) column: String,
    pub(crate) index: usize,
}
impl<T> Decoder for DirectDecoder<T>
where
    T: DecodeOwned,
{
    type Result = T;

    fn by_name<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
        row.get(self.column.as_str())
    }

    fn by_index<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
        row.get(self.index)
    }
}

/// A [`Decoder`] which "decodes" a value by using the [`Default`] trait
///
/// This is a "noop" which doesn't touch the [`&Row`](rorm_db::Row) at all
pub struct NoopDecoder<T>(pub(crate) PhantomData<T>);
impl<T> Decoder for NoopDecoder<T>
where
    T: Default,
{
    type Result = T;

    fn by_name<'index>(&'index self, _row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
        Ok(Default::default())
    }

    fn by_index<'index>(&'index self, _row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
        Ok(Default::default())
    }
}

macro_rules! decoder {
    ($($index:tt : $S:ident,)+) => {
        impl<$($S: Decoder),+> Decoder for ($($S,)+) {
            type Result = ($(
                $S::Result,
            )+);

            fn by_name<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
                Ok(($(
                    self.$index.by_name(row)?,
                )+))
            }

            fn by_index<'index>(&'index self, row: &'_ Row) -> Result<Self::Result, RowError<'index>> {
                Ok(($(
                    self.$index.by_index(row)?,
                )+))
            }
        }
    };
}
rorm_macro::impl_tuple!(decoder, 1..33);
