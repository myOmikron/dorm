//! Implementation detail of [`ForeignModelByField`]

use std::marker::PhantomData;

use rorm_db::sql::value::NullType;
use rorm_db::{Error, Row};

use crate::conditions::Value;
use crate::const_fn;
use crate::crud::decoder::Decoder;
use crate::fields::traits::{Array, FieldColumns};
use crate::fields::types::ForeignModelByField;
use crate::fields::utils::get_names::single_column_name;
use crate::internal::field::as_db_type::AsDbType;
use crate::internal::field::decoder::FieldDecoder;
use crate::internal::field::{Field, FieldProxy, FieldType, SingleColumnField};
use crate::internal::hmr;
use crate::internal::hmr::annotations::Annotations;
use crate::internal::hmr::db_type::DbType;
use crate::internal::hmr::Source;
use crate::internal::query_context::QueryContext;
use crate::internal::relation_path::Path;
use crate::model::{GetField, Model};
use crate::{impl_FieldEq, sealed};

impl<FF> FieldType for ForeignModelByField<FF>
where
    Self: ForeignModelTrait,
    FF: SingleColumnField,
    FF::Type: AsDbType,
    FF::Type: FieldType<Columns = Array<1>>,
    FF::Model: GetField<FF>, // always true
{
    type Columns = Array<1>;

    const NULL: FieldColumns<Self, NullType> = FF::Type::NULL;

    fn into_values(self) -> FieldColumns<Self, Value<'static>> {
        [FF::type_into_value(match self {
            ForeignModelByField::Key(value) => value,
            ForeignModelByField::Instance(model) => model.get_field(),
        })]
    }

    fn as_values(&self) -> FieldColumns<Self, Value<'_>> {
        [FF::type_as_value(match self {
            ForeignModelByField::Key(value) => value,
            ForeignModelByField::Instance(model) => model.borrow_field(),
        })]
    }

    type Decoder = ForeignModelByFieldDecoder<FF>;

    type GetAnnotations = foreign_annotations<Self>;

    type Check = <FF::Type as FieldType>::Check;

    type GetNames = single_column_name;
}

impl<FF> FieldType for Option<ForeignModelByField<FF>>
where
    Self: ForeignModelTrait,
    FF: SingleColumnField,
    FF::Type: AsDbType,
    FF::Type: FieldType<Columns = Array<1>>,
    FF::Model: GetField<FF>, // always true
    Option<FF::Type>: AsDbType,
{
    type Columns = Array<1>;

    const NULL: FieldColumns<Self, NullType> = FF::Type::NULL;

    fn into_values(self) -> FieldColumns<Self, Value<'static>> {
        self.map(ForeignModelByField::into_values)
            .unwrap_or([Value::Null(
                <<Option<FF::Type> as AsDbType>::DbType>::NULL_TYPE,
            )])
    }

    fn as_values(&self) -> FieldColumns<Self, Value<'_>> {
        self.as_ref()
            .map(ForeignModelByField::as_values)
            .unwrap_or([Value::Null(
                <<Option<FF::Type> as AsDbType>::DbType>::NULL_TYPE,
            )])
    }

    type Decoder = OptionForeignModelByFieldDecoder<FF>;

    type GetAnnotations = foreign_annotations<Self>;

    type Check = <FF::Type as FieldType>::Check;

    type GetNames = single_column_name;
}

#[doc(hidden)]
pub trait ForeignModelTrait {
    sealed!(trait);

    type DbType: DbType;
    type RelatedField: SingleColumnField;
    const IS_OPTION: bool;
    fn as_key(&self) -> Option<&<Self::RelatedField as Field>::Type>;
}

impl<FF> ForeignModelTrait for ForeignModelByField<FF>
where
    FF: SingleColumnField,
    FF::Type: AsDbType,
    FF::Model: GetField<FF>, // always true
{
    sealed!(impl);

    type DbType = <FF::Type as AsDbType>::DbType;
    type RelatedField = FF;
    const IS_OPTION: bool = false;
    fn as_key(&self) -> Option<&<Self::RelatedField as Field>::Type> {
        Some(match self {
            ForeignModelByField::Key(key) => key,
            ForeignModelByField::Instance(instance) => instance.borrow_field(),
        })
    }
}

impl<FF: SingleColumnField> ForeignModelTrait for Option<ForeignModelByField<FF>>
where
    FF: SingleColumnField,
    FF::Type: AsDbType,
    FF::Model: GetField<FF>, // always true
    Option<FF::Type>: AsDbType,
{
    sealed!(impl);

    type DbType = <FF::Type as AsDbType>::DbType;
    type RelatedField = FF;
    const IS_OPTION: bool = true;

    fn as_key(&self) -> Option<&<Self::RelatedField as Field>::Type> {
        self.as_ref().map(|value| match value {
            ForeignModelByField::Key(key) => key,
            ForeignModelByField::Instance(instance) => instance.borrow_field(),
        })
    }
}

const_fn! {
    /// - sets `nullable`
    /// - copies `max_length` from the foreign key
    /// - sets `foreign`
    pub fn foreign_annotations<T: ForeignModelTrait>(field: Annotations) -> [Annotations; 1] {
        let mut annos = field;
        annos.nullable = T::IS_OPTION;
        if annos.max_length.is_none() {
            let target_annos = <T::RelatedField as SingleColumnField>::EFFECTIVE_ANNOTATION;
            annos.max_length = target_annos.max_length;
        }
        annos.foreign = Some(hmr::annotations::ForeignKey {
            table_name: <T::RelatedField as Field>::Model::TABLE,
            column_name: <T::RelatedField as Field>::NAME,
        });
        [annos]
    }
}

/// Marker trait without actual bounds for fields of type foreign model
pub trait ForeignModelField: SingleColumnField {
    sealed!(trait);
}

pub(crate) type RF<F> = <<F as Field>::Type as ForeignModelTrait>::RelatedField;
impl<F> ForeignModelField for F
where
    F: SingleColumnField,
    F::Type: ForeignModelTrait,
    <<F::Type as ForeignModelTrait>::RelatedField as Field>::Model:
        GetField<<F::Type as ForeignModelTrait>::RelatedField>, // always true
{
    sealed!(impl);
}

/// [`FieldDecoder`] for [`ForeignModelByField<FF>`]
pub struct ForeignModelByFieldDecoder<FF: SingleColumnField>(<FF::Type as FieldType>::Decoder);
impl<FF: SingleColumnField> Decoder for ForeignModelByFieldDecoder<FF> {
    type Result = ForeignModelByField<FF>;

    fn by_name(&self, row: &Row) -> Result<Self::Result, Error> {
        self.0.by_name(row).map(ForeignModelByField::Key)
    }

    fn by_index(&self, row: &Row) -> Result<Self::Result, Error> {
        self.0.by_index(row).map(ForeignModelByField::Key)
    }
}
impl<FF: SingleColumnField> FieldDecoder for ForeignModelByFieldDecoder<FF> {
    fn new<F, P>(ctx: &mut QueryContext, _: FieldProxy<F, P>) -> Self
    where
        F: Field<Type = Self::Result>,
        P: Path,
    {
        Self(FieldDecoder::new(
            ctx,
            FieldProxy::<FakeFieldType<FF::Type, F>, P>::new(),
        ))
    }
}

/// [`FieldDecoder`] for [`Option<ForeignModelByField<FF>>`](ForeignModelByField)
pub struct OptionForeignModelByFieldDecoder<FF: SingleColumnField>(
    <Option<FF::Type> as FieldType>::Decoder,
)
where
    Option<FF::Type>: FieldType;
impl<FF: SingleColumnField> Decoder for OptionForeignModelByFieldDecoder<FF>
where
    Option<FF::Type>: FieldType,
{
    type Result = Option<ForeignModelByField<FF>>;

    fn by_name(&self, row: &Row) -> Result<Self::Result, Error> {
        self.0
            .by_name(row)
            .map(|option| option.map(ForeignModelByField::Key))
    }

    fn by_index(&self, row: &Row) -> Result<Self::Result, Error> {
        self.0
            .by_index(row)
            .map(|option| option.map(ForeignModelByField::Key))
    }
}
impl<FF: SingleColumnField> FieldDecoder for OptionForeignModelByFieldDecoder<FF>
where
    Option<FF::Type>: FieldType,
{
    fn new<F, P>(ctx: &mut QueryContext, _: FieldProxy<F, P>) -> Self
    where
        F: Field<Type = Self::Result>,
        P: Path,
    {
        Self(FieldDecoder::new(
            ctx,
            FieldProxy::<FakeFieldType<Option<FF::Type>, F>, P>::new(),
        ))
    }
}

/// Take a field `F` and create a new "fake" field with the different [`Field::Type`](Field::Type) `T`
#[allow(non_camel_case_types)]
struct FakeFieldType<T, F>(PhantomData<(T, F)>);
impl<T, F> Clone for FakeFieldType<T, F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, F> Copy for FakeFieldType<T, F> {}
impl<T, F> Field for FakeFieldType<T, F>
where
    T: FieldType + 'static,
    F: Field,
{
    type Type = T;
    type Model = F::Model;
    const INDEX: usize = F::INDEX;
    const NAME: &'static str = F::NAME;
    const EXPLICIT_ANNOTATIONS: Annotations = F::EXPLICIT_ANNOTATIONS;
    const SOURCE: Option<Source> = F::SOURCE;
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl_FieldEq!(
    impl<'rhs, FF> FieldEq<'rhs, FF::Type, FieldEq_ForeignModelByField_Owned> for ForeignModelByField<FF>
    where
        FF: SingleColumnField,
        FF::Type: AsDbType,
        FF::Type: FieldType<Columns = Array<1>>,
        FF::Model: GetField<FF>, // always true
    { <FF as SingleColumnField>::type_into_value }
);
impl_FieldEq!(
    impl<'rhs, FF> FieldEq<'rhs, FF::Type, FieldEq_ForeignModelByField_Owned> for Option<ForeignModelByField<FF>>
    where
        FF: SingleColumnField,
        FF::Type: AsDbType,
        FF::Type: FieldType<Columns = Array<1>>,
        Option<FF::Type>: AsDbType,
        FF::Model: GetField<FF>, // always true
    { <FF as SingleColumnField>::type_into_value }
);

impl_FieldEq!(
    impl<'rhs, FF> FieldEq<'rhs, &'rhs FF::Type, FieldEq_ForeignModelByField_Borrowed> for ForeignModelByField<FF>
    where
        FF: SingleColumnField,
        FF::Type: AsDbType,
        FF::Type: FieldType<Columns = Array<1>>,
        FF::Model: GetField<FF>, // always true
    { <FF as SingleColumnField>::type_as_value }
);
impl_FieldEq!(
    impl<'rhs, FF> FieldEq<'rhs, &'rhs FF::Type, FieldEq_ForeignModelByField_Borrowed> for Option<ForeignModelByField<FF>>
    where
        FF: SingleColumnField,
        FF::Type: AsDbType,
        FF::Type: FieldType<Columns = Array<1>>,
        Option<FF::Type>: AsDbType,
        FF::Model: GetField<FF>, // always true
    { <FF as SingleColumnField>::type_as_value }
);

#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct FieldEq_ForeignModelByField_Owned;
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct FieldEq_ForeignModelByField_Borrowed;
