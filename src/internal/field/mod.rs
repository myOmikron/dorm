//! The various field traits and their proxy.
//!
//! # Introduction
//! Rorm's main entry point is the [`Model`] trait and its derive macro.
//! It takes a struct and generates the code to represent this struct as a database table.
//! To do so each of the struct's fields need to be represented in some way.
//!
//! For each field the derive macro declares a unit struct (i.e. an empty struct) to represent it.
//! This empty struct is then "populated" with the field's information using various traits defined in this module.
//!
//! # Trait Implementation Flow
//! As stated in the introduction, the derive macro generates an unit struct per field.
//! It the proceeds to implement then [`RawField`] trait on this empty struct.
//! Therefore, [`RawField`] encapsulates all information the macro can gather.
//! This includes:
//! - the name (a db safe version of it, to be precise)
//! - its "raw type" ("raw" because the macro can't make any deductions about the type)
//! - the various annotations inside a `#[rorm(...)]` attribute
//!
//! #### Small illustration
//! ```text
//! #[derive(Model)]
//! struct User {
//!     id: i32,
//!     ...
//! }
//! ```
//! will produce something like
//! ```text
//! struct __User_id;
//! impl RawField for __User_id {
//!     type RawType = i32;
//!     const NAME: &'static str = "id";
//!     ...
//! }
//! ```
//!
//! ---
//!
//! From there on, further traits are implemented using generic `impl`s defined in this module.
//! These implementations branch depending on the field's type.
//!
//! **This hits a limitation in rust:**
//! We need to provide different generic implementations for the same traits ([`AbstractField`] and [`Field`]).
//! rust enforces implementations to don't overlap.
//! To achieve this a [`FieldKind`] is introduced.
//! Each [`FieldType`] (a type usable as a field) is of exactly one such kind.
//! Using this kinds as constraints for the generic [`RawField]'s type,
//! should make these implementation branches mutually exclusive.
//! However rust doesn't quite understand this, which is due to an old bug (stated by some online sources).
//!
//! As a workaround all traits after [`RawField`] carry a generic [`FieldKind`] which defaults to `<Self as RawField>::Kind`.
//! This way
//! - The traits (for example `Field<kind::AsDbType` and `Field<kind::ForeignModel>`)
//! are treated as different traits, as far as the impl overlap is concerned.
//! - You can write `F: Field` in constraint without having to state the generic every time.
//!
//! *(Thank you a lot to whomever's blog post I read to figure all this out.
//! I'm sorry, I couldn't find you anymore to credit you properly.)*
//!
//! ---
//!
//! **The concrete branches are experimental and might change any time!**
//!
//! The [`Field`] implementation does further processing of [`RawField`].
//! For example it merges the annotations set by the user with annotations implied by the raw type
//! and runs a linter shared with `rorm-cli` on them.

use std::marker::PhantomData;
use std::mem::ManuallyDrop;

use rorm_db::row::DecodeOwned;
use rorm_declaration::imr;

use crate::conditions::Value;
use crate::internal::hmr::annotations::Annotations;
use crate::internal::hmr::db_type::DbType;
use crate::internal::hmr::{AsImr, Source};
use crate::internal::relation_path::{Path, PathImpl, PathStep, ResolvedRelatedField};
use crate::model::{ConstNew, Model};
use crate::{const_panic, sealed};

pub mod access;
pub mod as_db_type;
pub mod decoder;
pub mod foreign_model;
pub mod modifier;

use as_db_type::AsDbType;

use crate::fields::traits::FieldType;
use crate::internal::array_utils::IntoArray;
use crate::internal::field::modifier::AnnotationsModifier;

/// Marker trait for various kinds of fields
pub trait FieldKind {
    sealed!(trait);
}
/// Namespace for the different [`FieldKind`] impls.
pub mod kind {
    use super::FieldKind;
    use crate::sealed;

    /// Marker for some field which is a [`ForeignModel`](crate::fields::types::ForeignModelByField)
    pub struct ForeignModel;
    /// Marker for some field which is a [`BackRef`](crate::fields::types::BackRef)
    pub struct BackRef;
    /// Marker for some field which is an [`AsDbType`](crate::internal::field::as_db_type::AsDbType)
    pub struct AsDbType;
    /// Marker for some field which is an [`DateTime<FixedOffset>`](chrono::DateTime)
    pub struct DateTime;

    impl FieldKind for ForeignModel {
        sealed!(impl);
    }
    impl FieldKind for BackRef {
        sealed!(impl);
    }
    impl FieldKind for AsDbType {
        sealed!(impl);
    }
    impl FieldKind for DateTime {
        sealed!(impl);
    }
}

/// This trait is implemented by the `#[derive(Model)]` macro on unique unit struct for each of a model's fields.
///
/// It contains all the information a model's author provides on the field.
///
/// This trait itself doesn't do much, but it forms the basis to implement the other traits.
pub trait RawField: 'static + Copy {
    /// The field's kind which is determined by its [type](RawField::Type)
    type Kind: FieldKind;

    /// The type stored in the model's field
    type Type: FieldType<Kind = Self::Kind>;

    /// The model this field is part of
    type Model: Model;

    /// This field's position in the model
    const INDEX: usize;

    /// A db safe name of this field
    const NAME: &'static str;

    /// List of annotations which were set by the user
    const EXPLICIT_ANNOTATIONS: Annotations;

    /// List of annotations which are passed to db, if this field is a single column
    const EFFECTIVE_ANNOTATIONS: Option<Annotations> =
        <Self::Type as FieldType>::AnnotationsModifier::<Self>::MODIFIED;

    /// Optional definition of the location of field in the source code
    const SOURCE: Option<Source>;

    /// Create a new instance
    ///
    /// Since `Self` is always a zero sized type, this is a noop.
    /// It exists to enable accessing field method through [`FieldProxy`] without having to forward every one.
    fn new() -> Self;
}

/// A field which is stored in db via a single column
pub trait SingleColumnField: RawField {
    /// Borrow an instance of the field's type as a [`Value`]
    fn type_as_value(field: &Self::Type) -> Value;

    /// Convert an instance of the field's type into a static [`Value`]
    fn type_into_value(field: Self::Type) -> Value<'static>;
}
impl<F> SingleColumnField for F
where
    F: RawField,
    for<'a> <F::Type as FieldType>::Columns<Value<'a>>: IntoArray<1>,
{
    fn type_as_value(field: &Self::Type) -> Value {
        let [value] = field.as_values().into_array();
        value
    }

    fn type_into_value(field: Self::Type) -> Value<'static> {
        let [value] = field.into_values().into_array();
        value
    }
}

/// A [`RawField`] which represents a single column in the database
pub trait Field<K: FieldKind = <Self as RawField>::Kind>: RawField {
    sealed!(trait);

    /// The data type as which this field is stored in the db
    type DbType: DbType;

    /// Entry point for compile time checks on a single field
    ///
    /// It is "used" in [`FieldProxy::new`] to force the compiler to evaluate it.
    const CHECK: usize = {
        let Some(annotations) = Self::EFFECTIVE_ANNOTATIONS else {
            panic!()
        };

        // Are required annotations set?
        let mut required = Self::DbType::REQUIRED;
        while let [head, tail @ ..] = required {
            required = tail;
            if !annotations.is_set(head) {
                const_panic!(&[
                    Self::Model::TABLE,
                    ".",
                    Self::NAME,
                    " requires the annotation `",
                    head.as_str(),
                    "` but it's missing",
                ]);
            }
        }

        // Run the annotations lint shared with rorm-cli
        let annotations = annotations.as_lint();
        if let Err(err) = annotations.check() {
            const_panic!(&[
                Self::Model::TABLE,
                ".",
                Self::NAME,
                " has invalid annotations: ",
                err,
            ]);
        }

        Self::INDEX
    };

    /// A type which can be retrieved from the db and then converted into [`Self::Type`](RawField::Type).
    type Primitive: DecodeOwned;

    /// Convert the associated primitive type into [`Self::Type`](RawField::Type).
    #[allow(clippy::wrong_self_convention)] // Self is not part of the conversion and just there for easier access via macros
    fn from_primitive(self, primitive: Self::Primitive) -> Self::Type;
}

impl<T: AsDbType, F: RawField<Type = T, Kind = kind::AsDbType>> Field<kind::AsDbType> for F {
    sealed!(impl);

    type DbType = <T as AsDbType>::DbType;

    type Primitive = T::Primitive;

    fn from_primitive(self, primitive: Self::Primitive) -> Self::Type {
        T::from_primitive(primitive)
    }
}

/// A common interface unifying the fields of various kinds.
pub trait AbstractField<K: FieldKind = <Self as RawField>::Kind>: RawField {
    sealed!(trait);

    /// Add the field to its model's intermediate model representation
    ///
    /// - [`kind::BackRef`] fields don't add anything
    /// - [`Field`] fields add their database column
    /// - there are plans to add fields which might map to more than one database column.
    fn push_imr(self, imr: &mut Vec<imr::Field>);

    /// The columns' names which store this field
    const COLUMNS: &'static [&'static str] = &[];
}
macro_rules! impl_abstract_from_field {
    ($kind:ty) => {
        impl<F: Field<$kind>> AbstractField<$kind> for F {
            sealed!(impl);

            fn push_imr(self, imr: &mut Vec<imr::Field>) {
                imr.push(imr::Field {
                    name: F::NAME.to_string(),
                    db_type: F::DbType::IMR,
                    annotations: Self::EFFECTIVE_ANNOTATIONS
                        .unwrap_or_else(Annotations::empty)
                        .as_imr(),
                    source_defined_at: F::SOURCE.map(|s| s.as_imr()),
                });
            }

            const COLUMNS: &'static [&'static str] = &[F::NAME];
        }
    };
}
impl_abstract_from_field!(kind::AsDbType);
impl_abstract_from_field!(kind::ForeignModel);

/// This struct acts as a proxy exposing type level information from the [`RawField`] trait on the value level.
///
/// On top of that it can be used to keep track of the "path" this field is accessed through, when dealing with relations.
///
/// ## Type as Value
/// In other words [`FieldProxy`] allows access to things like [`RawField::NAME`] without access to the concrete field type.
///
/// Pseudo code for illustration:
/// ```skip
/// // The following is a rough sketch of what the #[derive(Model)] will do:
/// pub struct Id;
/// impl Field for Id {
///     ...
/// }
///
/// pub struct Fields {
///     pub id: FieldProxy<Id>,
///     ...
/// }
///
/// pub struct User {
///     pub id: i32,
/// }
/// impl Model for User {
///     type Fields = Fields;
///     const FIELDS: Self::Fields = Fields {
///         id: Id,
///         ...
///     }
/// }
///
/// // To access Id::NAME from user code, we can't use the Field trait itself,
/// // because the type Id is not really accessible. (It's been generated from a macro.)
/// // Also User::FIELDS or User::F should have more of a struct like syntax.
/// //
/// // So, the Fields struct holds FieldProxy<Id> instead of Id, which implements simple methods
/// // forwarding varies data and behaviors from Id:
///
/// Id::NAME ~ User::F.id.name()
/// Id::Index ~ User::F.id.index()
/// Id::Type::from_primitive ~ User::F.id.convert_primitive
/// ```
pub struct FieldProxy<Field, Path>(PhantomData<ManuallyDrop<(Field, Path)>>);

// SAFETY:
// struct contains no data
unsafe impl<F, P> Send for FieldProxy<F, P> {}

impl<F: RawField, P> FieldProxy<F, P> {
    /// Create a new instance
    pub const fn new() -> Self {
        Self(PhantomData)
    }

    /// Get the field's position in the Model
    pub const fn index(_field: Self) -> usize {
        F::INDEX
    }

    /// Change the path
    pub const fn through<NewP>(self) -> FieldProxy<F, NewP> {
        FieldProxy::new()
    }
}
impl<F: AbstractField, P> FieldProxy<F, P> {
    /// Get the names of the columns which store the field
    pub const fn columns(_field: Self) -> &'static [&'static str] {
        F::COLUMNS
    }

    /// Get the underlying field to call its methods
    pub fn field(&self) -> F {
        F::new()
    }
}
impl<Field, Path> Clone for FieldProxy<Field, Path> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Field, Path> Copy for FieldProxy<Field, Path> {}

/// A field whose proxy should implement [`Deref`](std::ops::Deref) to some collection of fields.
///
/// Depending on the field, this collection might differ in meaning
/// - For [`BackRef`](crate::fields::types::BackRef) and [`ForeignModel`](crate::fields::types::ForeignModelByField),
///   its their related model's fields
/// - For multi-column fields, its their "contained" fields
pub trait ContainerField<P: Path, K: FieldKind = <Self as RawField>::Kind>: RawField {
    /// Struct of contained fields
    type Target: ConstNew;
}

impl<F: RawField, P: Path> std::ops::Deref for FieldProxy<F, P>
where
    F: ContainerField<P>,
{
    type Target = F::Target;

    fn deref(&self) -> &'static Self::Target {
        ConstNew::REF
    }
}

impl<F, P> ContainerField<P, kind::ForeignModel> for F
where
    P: Path,
    F: RawField<Kind = kind::ForeignModel>,
    PathStep<F, P>: PathImpl<F::Type>,
{
    type Target =
        <<ResolvedRelatedField<F, P> as RawField>::Model as Model>::Fields<PathStep<F, P>>;
}

impl<F, P> ContainerField<P, kind::BackRef> for F
where
    P: Path,
    F: RawField<Kind = kind::BackRef>,
    PathStep<F, P>: PathImpl<F::Type>,
{
    type Target =
        <<ResolvedRelatedField<F, P> as RawField>::Model as Model>::Fields<PathStep<F, P>>;
}
