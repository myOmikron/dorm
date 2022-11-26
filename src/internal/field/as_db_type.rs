//! defines and implements the [AsDbType] trait.

use rorm_db::row::DecodeOwned;
use rorm_declaration::imr;

use crate::conditions::Value;
use crate::internal::field::Field;
use crate::internal::hmr;
use crate::internal::hmr::annotations::Annotations;

/// This trait maps rust types to database types
///
/// I.e. it specifies which datatypes are allowed on model's fields.
///
/// The mysterious generic `F` which appears in some places, is a workaround.
/// [ForeignModel] requires the [Field] it is the type of, in order to access its [RelatedField].
/// Instead of creating a whole new process to define a [Field] which has a [RawType] of [ForeignModel],
/// the places which require the [RelatedField] forward the [Field] via this generic `F`.
///
/// [RawType]: crate::internal::field::RawField::RawType
/// [ForeignModel]: crate::internal::field::foreign_model::ForeignModel
/// [RelatedField]: crate::internal::field::RawField::RelatedField
pub trait AsDbType {
    /// A type which can be retrieved from the db and then converted into Self.
    type Primitive: DecodeOwned;

    /// The database type as defined in the Intermediate Model Representation
    type DbType<F: Field>: hmr::db_type::DbType;

    /// Annotations implied by this type
    const IMPLICIT: Option<Annotations> = None;

    /// Convert the associated primitive type into `Self`.
    ///
    /// This function allows "non-primitive" types like any [DbEnum] to implement
    /// their decoding without access to the underlying db details (namely `sqlx::Decode`)
    fn from_primitive(primitive: Self::Primitive) -> Self;

    /// Convert a reference to `Self` into the primitive [`Value`] used by our db implementation.
    fn as_primitive<F: Field>(&self) -> Value;

    /// Whether this type supports null.
    ///
    /// This will be mapped to NotNull in the imr.
    const IS_NULLABLE: bool = false;

    /// Add type specific imr annotations.
    ///
    /// Currently only used by [ForeignModel](super::foreign_model::ForeignModel).
    fn custom_annotations<F: Field>(_annotations: &mut Vec<imr::Annotation>) {}

    /// Whether this type is a [ForeignModel](super::foreign_model::ForeignModel).
    ///
    /// This is only required when creating the linters annotations struct.
    /// [custom_annotations](AsDbType::custom_annotations) is used when generating the imr annotations.
    const IS_FOREIGN: bool = false;
}

macro_rules! impl_as_db_type {
    ($type:ty, $db_type:ident, $value_variant:ident $(using $method:ident)?) => {
        impl AsDbType for $type {
            type Primitive = Self;

            type DbType<F: Field> = hmr::db_type::$db_type;

            #[inline(always)]
            fn from_primitive(primitive: Self::Primitive) -> Self {
                primitive
            }

            impl_as_db_type!(impl_as_primitive, $type, $db_type, $value_variant $(using $method)?);
        }
    };
    (impl_as_primitive, $type:ty, $db_type:ident, $value_variant:ident) => {
        #[inline(always)]
        fn as_primitive<F: Field>(&self) -> Value {
            Value::$value_variant(*self)
        }
    };
    (impl_as_primitive, $type:ty, $db_type:ident, $value_variant:ident using $method:ident) => {
        #[inline(always)]
        fn as_primitive<F: Field>(&self) -> Value {
            Value::$value_variant(self.$method())
        }
    };
}
impl_as_db_type!(chrono::NaiveTime, Time, NaiveTime);
impl_as_db_type!(chrono::NaiveDateTime, DateTime, NaiveDateTime);
impl_as_db_type!(chrono::NaiveDate, Date, NaiveDate);
impl_as_db_type!(i16, Int16, I16);
impl_as_db_type!(i32, Int32, I32);
impl_as_db_type!(i64, Int64, I64);
impl_as_db_type!(f32, Float, F32);
impl_as_db_type!(f64, Double, F64);
impl_as_db_type!(bool, Boolean, Bool);
impl_as_db_type!(Vec<u8>, VarBinary, Binary using as_slice);
impl_as_db_type!(String, VarChar, String using as_str);

impl<T: AsDbType> AsDbType for Option<T> {
    type Primitive = Option<T::Primitive>;
    type DbType<F: Field> = T::DbType<F>;

    const IMPLICIT: Option<Annotations> = T::IMPLICIT;

    fn from_primitive(primitive: Self::Primitive) -> Self {
        primitive.map(T::from_primitive)
    }

    fn as_primitive<F: Field>(&self) -> Value {
        match self {
            Some(value) => value.as_primitive::<F>(),
            None => Value::Null,
        }
    }

    const IS_NULLABLE: bool = true;

    fn custom_annotations<F: Field>(annotations: &mut Vec<imr::Annotation>) {
        T::custom_annotations::<F>(annotations);
    }

    const IS_FOREIGN: bool = T::IS_FOREIGN;
}
