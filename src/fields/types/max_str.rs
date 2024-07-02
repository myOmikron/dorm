use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

use rorm_db::sql::value::NullType;
use rorm_db::{Error, Row};
use rorm_declaration::imr;
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::conditions::Value;
use crate::crud::decoder::Decoder;
use crate::fields::traits::FieldType;
use crate::fields::types::max_str_impl::{LenImpl, NumBytes};
use crate::impl_FieldEq;
use crate::internal::field::as_db_type::{get_single_imr, AsDbType};
use crate::internal::field::decoder::FieldDecoder;
use crate::internal::field::modifier::{MergeAnnotations, SingleColumnCheck, SingleColumnFromName};
use crate::internal::field::{Field, FieldProxy};
use crate::internal::hmr;
use crate::internal::hmr::annotations::{Annotations, MaxLength};
use crate::internal::query_context::QueryContext;
use crate::internal::relation_path::Path;

/// String which is restricted to a maximum length
///
/// When storing strings in a database you have to specify a `#[rorm(max_length = ...)]`
/// which is enforced by the database upon insertion or update.
/// This can result in a rather opaque [`rorm::Error`](crate::Error) when you
/// fail to check your strings before passing to the database.
/// This type forces you to perform this check before the database by having a fallible constructor.
///
/// The "length" of a string is not really a well-defined thing
/// and different databases might have different opinions.
/// So this type uses a generic `Impl: LenImpl` to select our databases definition of "length".
/// However, note that this will reduce our code's portability and is therefor not the recommended default.
///
/// This type is also generic over the string implementation to also support `&str` and `Cow<'_, str>`.
#[derive(Copy, Clone, Debug)]
pub struct MaxStr<const MAX_LEN: usize = 255, Impl = NumBytes, Str = String> {
    string: Str,
    len_impl: Impl,
}

impl<const MAX_LEN: usize, Impl, Str> Default for MaxStr<MAX_LEN, Impl, Str>
where
    Self: Sized,
    Str: Deref<Target = str> + Default,
    Impl: LenImpl + Default,
{
    /// Returns the “default value” for a type. [Read more](Default::default)
    ///
    /// # Panics
    /// If [`Str::default`] produces a value which is longer than `MAX_LEN`.
    fn default() -> Self {
        Self::new(Default::default())
            .unwrap_or_else(|_| panic!("A `Default` for a `Deref<Target = str>` should be empty"))
    }
}

impl<const MAX_LEN: usize, Impl, Str> MaxStr<MAX_LEN, Impl, Str>
where
    Str: Deref<Target = str>,
    Impl: LenImpl,
{
    /// Wraps a string returning `Err` if it is too long.
    pub fn new(string: Str) -> Result<Self, MaxLenError<Str>>
    where
        Impl: Default,
    {
        Self::with_impl(string, Impl::default())
    }

    /// Wraps a string using a custom [`LenImpl`] returning `None` if the string is too long.
    pub fn with_impl(string: Str, len_impl: Impl) -> Result<Self, MaxLenError<Str>> {
        let got = len_impl.len(&string);
        if got > MAX_LEN {
            Err(MaxLenError {
                string,
                max: MAX_LEN,
                got,
            })
        } else {
            Ok(Self { string, len_impl })
        }
    }

    /// Returns the length of the wrapped `Str`.
    ///
    /// This method replaces `str::len` which is exposed through `Deref<Target = str>`
    /// to return the length relevant to the limit.
    pub fn len(&self) -> usize {
        self.len_impl.len(&self.string)
    }

    /// Borrow the wrapped string while remembering its `MAX_LEN`.
    pub fn as_ref(&self) -> MaxStr<MAX_LEN, &Impl, &str> {
        MaxStr {
            string: &self.string,
            len_impl: &self.len_impl,
        }
    }

    /// Borrow the wrapped string as `&str`
    pub fn as_str(&self) -> &str {
        &self.string
    }

    /// Unwraps the inner string
    pub fn into_inner(self) -> Str {
        self.string
    }
}

impl<const MAX_LEN: usize, Impl, Str> fmt::Display for MaxStr<MAX_LEN, Impl, Str>
where
    Str: Deref<Target = str>,
    Impl: LenImpl,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.string)
    }
}

/// Error returned by [`MaxStr`]'s constructors when the input string is too long
#[derive(Debug)]
pub struct MaxLenError<Str = String> {
    /// The rejected string
    pub string: Str,
    /// The maximum length which was exceeded
    pub max: usize,
    /// The `string`'s length (according to the [`LenImpl`] which was used)
    pub got: usize,
}

impl<const MAX_LEN: usize, Impl, Str> Deref for MaxStr<MAX_LEN, Impl, Str>
where
    Str: Deref<Target = str>,
{
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

impl<const MAX_LEN: usize, Impl, Str> Serialize for MaxStr<MAX_LEN, Impl, Str>
where
    Str: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.string.serialize(serializer)
    }
}

impl<'de, const MAX_LEN: usize, Impl, Str> Deserialize<'de> for MaxStr<MAX_LEN, Impl, Str>
where
    Str: Deref<Target = str> + Deserialize<'de>,
    Impl: LenImpl + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(Str::deserialize(deserializer)?).map_err(|error| {
            <D::Error as serde::de::Error>::invalid_value(
                Unexpected::Str(&error.string),
                &format!("string with a maximum length of {MAX_LEN}").as_str(),
            )
        })
    }
}

impl<const MAX_LEN: usize, Impl> AsDbType for MaxStr<MAX_LEN, Impl, String>
where
    Impl: LenImpl + Default + 'static,
{
    type Primitive = String;
    type DbType = hmr::db_type::VarChar;
    const IMPLICIT: Option<Annotations> = Some(Annotations {
        max_length: Some(MaxLength(MAX_LEN as i32)),
        ..Annotations::empty()
    });
}

impl<const MAX_LEN: usize, Impl> AsDbType for Option<MaxStr<MAX_LEN, Impl, String>>
where
    Impl: LenImpl + Default + 'static,
{
    type Primitive = String;
    type DbType = hmr::db_type::VarChar;
    const IMPLICIT: Option<Annotations> = Some(Annotations {
        max_length: Some(MaxLength(MAX_LEN as i32)),
        nullable: true,
        ..Annotations::empty()
    });
}

impl<const MAX_LEN: usize, Impl> FieldType for MaxStr<MAX_LEN, Impl, String>
where
    Impl: LenImpl + Default + 'static,
{
    type Columns<T> = [T; 1];

    fn into_values(self) -> Self::Columns<Value<'static>> {
        [Value::String(Cow::Owned(self.string))]
    }

    fn as_values(&self) -> Self::Columns<Value<'_>> {
        [Value::String(Cow::Borrowed(&self.string))]
    }

    fn get_imr<F: Field<Type = Self>>() -> Self::Columns<imr::Field> {
        get_single_imr::<F>(imr::DbType::VarChar)
    }

    type Decoder = MaxStrDecoder<MAX_LEN, Impl>;
    type AnnotationsModifier<F: Field<Type = Self>> = MergeAnnotations<Self>;
    type CheckModifier<F: Field<Type = Self>> = SingleColumnCheck<hmr::db_type::VarChar>;
    type ColumnsFromName<F: Field<Type = Self>> = SingleColumnFromName;
}

pub struct MaxStrDecoder<const MAX_LEN: usize, Impl: LenImpl> {
    column: String,
    index: usize,
    generics: PhantomData<Impl>,
}

impl<const MAX_LEN: usize, Impl> Decoder for MaxStrDecoder<MAX_LEN, Impl>
where
    Impl: LenImpl + Default,
{
    type Result = MaxStr<MAX_LEN, Impl, String>;

    fn by_name(&self, row: &Row) -> Result<Self::Result, Error> {
        MaxStr::<MAX_LEN, Impl, String>::new(row.get(self.column.as_str())?)
            .map_err(|_| Error::DecodeError(format!("string is longer than {MAX_LEN}")))
    }

    fn by_index(&self, row: &Row) -> Result<Self::Result, Error> {
        MaxStr::<MAX_LEN, Impl, String>::new(row.get(self.index)?)
            .map_err(|_| Error::DecodeError(format!("string is longer than {MAX_LEN}")))
    }
}

impl<const MAX_LEN: usize, Impl> FieldDecoder for MaxStrDecoder<MAX_LEN, Impl>
where
    Impl: LenImpl + Default,
{
    fn new<F, P>(ctx: &mut QueryContext, _: FieldProxy<F, P>) -> Self
    where
        F: Field<Type = Self::Result>,
        P: Path,
    {
        let (index, column) = ctx.select_field::<F, P>();
        Self {
            column,
            index,
            generics: PhantomData,
        }
    }
}

impl<const MAX_LEN: usize, Impl> FieldType for Option<MaxStr<MAX_LEN, Impl, String>>
where
    Impl: LenImpl + Default + 'static,
{
    type Columns<T> = [T; 1];

    fn into_values(self) -> Self::Columns<Value<'static>> {
        [if let Some(string) = self {
            Value::String(Cow::Owned(string.string))
        } else {
            Value::Null(NullType::String)
        }]
    }

    fn as_values(&self) -> Self::Columns<Value<'_>> {
        [if let Some(string) = self {
            Value::String(Cow::Borrowed(&string.string))
        } else {
            Value::Null(NullType::String)
        }]
    }

    fn get_imr<F: Field<Type = Self>>() -> Self::Columns<imr::Field> {
        get_single_imr::<F>(imr::DbType::VarChar)
    }

    type Decoder = OptionMaxStrDecoder<MAX_LEN, Impl>;
    type AnnotationsModifier<F: Field<Type = Self>> = MergeAnnotations<Self>;
    type CheckModifier<F: Field<Type = Self>> = SingleColumnCheck<hmr::db_type::VarChar>;
    type ColumnsFromName<F: Field<Type = Self>> = SingleColumnFromName;
}

pub struct OptionMaxStrDecoder<const MAX_LEN: usize, Impl: LenImpl> {
    column: String,
    index: usize,
    generics: PhantomData<Impl>,
}

impl<const MAX_LEN: usize, Impl> Decoder for OptionMaxStrDecoder<MAX_LEN, Impl>
where
    Impl: LenImpl + Default,
{
    type Result = Option<MaxStr<MAX_LEN, Impl, String>>;

    fn by_name(&self, row: &Row) -> Result<Self::Result, Error> {
        row.get::<Option<String>, _>(self.column.as_str())?
            .map(|string| {
                MaxStr::<MAX_LEN, Impl, String>::new(string)
                    .map_err(|_| Error::DecodeError(format!("string is longer than {MAX_LEN}")))
            })
            .transpose()
    }

    fn by_index(&self, row: &Row) -> Result<Self::Result, Error> {
        row.get::<Option<String>, _>(self.index)?
            .map(|string| {
                MaxStr::<MAX_LEN, Impl, String>::new(string)
                    .map_err(|_| Error::DecodeError(format!("string is longer than {MAX_LEN}")))
            })
            .transpose()
    }
}

impl<const MAX_LEN: usize, Impl> FieldDecoder for OptionMaxStrDecoder<MAX_LEN, Impl>
where
    Impl: LenImpl + Default,
{
    fn new<F, P>(ctx: &mut QueryContext, _: FieldProxy<F, P>) -> Self
    where
        F: Field<Type = Self::Result>,
        P: Path,
    {
        let (index, column) = ctx.select_field::<F, P>();
        Self {
            column,
            index,
            generics: PhantomData,
        }
    }
}

impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, &'rhs str> for MaxStr<MAX_LEN, Impl> { conv_string });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, &'rhs String> for MaxStr<MAX_LEN, Impl> { conv_string });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, String> for MaxStr<MAX_LEN, Impl> { conv_string });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, Cow<'rhs, str>> for MaxStr<MAX_LEN, Impl> { conv_string });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, Option<&'rhs str>> for Option<MaxStr<MAX_LEN, Impl>> { |option: Option<_>| option.map(conv_string).unwrap_or(Value::Null(NullType::String)) });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, Option<&'rhs String>> for Option<MaxStr<MAX_LEN, Impl>> { |option: Option<_>| option.map(conv_string).unwrap_or(Value::Null(NullType::String)) });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, Option<String>> for Option<MaxStr<MAX_LEN, Impl>> { |option: Option<_>| option.map(conv_string).unwrap_or(Value::Null(NullType::String)) });
impl_FieldEq!(impl<'rhs, const MAX_LEN: usize, Impl> FieldEq<'rhs, Option<Cow<'rhs, str>>> for Option<MaxStr<MAX_LEN, Impl>> { |option: Option<_>| option.map(conv_string).unwrap_or(Value::Null(NullType::String)) });
fn conv_string<'a>(value: impl Into<Cow<'a, str>>) -> Value<'a> {
    Value::String(value.into())
}

#[cfg(feature = "utoipa")]
mod utoipa_impl {
    use utoipa::openapi::{Object, RefOr, Schema, SchemaType};
    use utoipa::ToSchema;

    use crate::fields::types::max_str_impl::LenImpl;
    use crate::fields::types::MaxStr;

    impl<'s, const MAX_LEN: usize, Impl: LenImpl> ToSchema<'s> for MaxStr<MAX_LEN, Impl, String> {
        fn schema() -> (&'s str, RefOr<Schema>) {
            (
                "MaxStr",
                RefOr::T(Schema::Object(Object::with_type(SchemaType::String))),
            )
        }
    }
}
