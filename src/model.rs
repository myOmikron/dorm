use crate::conditions::{Binary, BinaryOperator, Column, Value};
use crate::internal::field::as_db_type::AsDbType;
use crate::internal::field::{Field, Identical, RawField};
use crate::internal::relation_path::Path;
use rorm_db::row::FromRow;
use rorm_declaration::imr;

/// Trait implemented on Patches i.e. a subset of a model's fields.
///
/// Implemented by [`derive(Patch)`] as well as [`derive(Model)`].
pub trait Patch: FromRow + 'static {
    /// The model this patch is for
    type Model: Model;

    /// List of columns i.e. fields this patch contains
    const COLUMNS: &'static [Option<&'static str>];

    /// List of fields' indexes this patch contains
    ///
    /// Used in [`contains_index`]
    const INDEXES: &'static [usize];

    /// Get a field's db value by its index
    fn get_value(&self, index: usize) -> Option<Value>;
}

/// The [Condition](crate::conditions::Condition) type returned by [Patch::as_condition]
pub type PatchAsCondition<'a, P> =
    Binary<Column<<<P as Patch>::Model as Model>::Primary, <P as Patch>::Model>, Value<'a>>;

/// Check whether a [`Patch`] contains a certain field index.
///
/// This function in const and can therefore check the existence of fields at compile time.
pub const fn contains_index<P: Patch>(field: usize) -> bool {
    let mut indexes = P::INDEXES;
    while let [index, remaining @ ..] = indexes {
        indexes = remaining;
        if *index == field {
            return true;
        }
    }
    false
}

/// Create an iterator from a patch which yield its fields as db values
///
/// This method can't be part of the [`Patch`] trait, since `impl Trait` is not allowed in traits.
pub fn iter_columns<P: Patch>(patch: &P) -> impl Iterator<Item = Value> {
    P::INDEXES
        .iter()
        .filter_map(|&index| patch.get_value(index))
}

/// Trait implementing most database interactions for a struct.
///
/// It should only ever be generated using [`derive(Model)`](rorm_macro::Model).
pub trait Model: Patch<Model = Self> {
    /// The primary key
    type Primary: Field<Model = Self>;

    /// A struct which "maps" field identifiers their descriptions (i.e. [`Field<T>`](crate::internal::field::Field)).
    ///
    /// The struct is constructed once in the [`Model::FIELDS`] constant.
    type Fields<P: Path>: ConstNew;

    /// A constant struct which "maps" field identifiers their descriptions (i.e. [`Field<T>`](crate::internal::field::Field)).
    const FIELDS: Self::Fields<Self> = Self::Fields::NEW;

    /// Shorthand version of [`FIELDS`]
    ///
    /// [`FIELDS`]: Model::FIELDS
    const F: Self::Fields<Self> = Self::Fields::NEW;

    /// The model's table name
    const TABLE: &'static str;

    /// Returns the model's intermediate representation
    ///
    /// As library user you probably won't need this. You might want to look at [`write_models`].
    ///
    /// [`write_models`]: crate::write_models
    fn get_imr() -> imr::Model;
}

/// Expose a models' fields on the type level using indexes
pub trait FieldByIndex<const INDEX: usize>: Model {
    /// The model's field at `INDEX`
    type Field: RawField<Model = Self>;
}

/// Generic access to a patch's fields
pub trait GetField<F>: Patch
where
    F: RawField<Model = Self::Model>,
{
    /// Get reference to the field
    fn get_field(&self) -> &F::RawType;

    /// Get mutable reference to the field
    fn get_field_mut(&mut self) -> &mut F::RawType;
}

/// Update a model's field based on the model's primary key
pub trait UpdateField<F: RawField<Model = Self>>: Model {
    /// Update a model's field based on the model's primary key
    fn update_field<'m, T>(
        &'m mut self,
        update: impl FnOnce(&'m <<Self as Model>::Primary as Field>::Type, &'m mut F::RawType) -> T,
    ) -> T;
}

/// exposes a `NEW` constant, which act like [Default::default] but constant.
///
/// It's workaround for not having const methods in traits
pub trait ConstNew {
    /// A new or default instance
    const NEW: Self;
}
