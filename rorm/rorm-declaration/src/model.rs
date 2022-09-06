use std::ops::{Deref, DerefMut};

use crate::imr;

/// This trait maps rust types to database types
pub trait AsDbType {
    /// Returns the database type as defined in the Intermediate Model Representation
    ///
    /// This function takes a fields annotations because they might change the db's datatype.
    /// For example a [`String`] with the choices attribute will become [`Choices`] instead of [`VarChar`].
    ///
    /// [`Choices`]: imr::DbType::Choices
    /// [`VarChar`]: imr::DbType::VarChar
    fn as_db_type(annotations: &[imr::Annotation]) -> imr::DbType;

    /// Returns a list of migrator annotations which are implied by the type.
    ///
    /// For most types this would be empty. So that's its default implementation.
    /// It is called after `as_db_type` and therefore not available to it.
    fn implicit_annotations() -> Vec<imr::Annotation> {
        Vec::new()
    }

    /// Return whether this type supports null.
    ///
    /// (The only type returning true is an [`Option`])
    fn is_nullable() -> bool {
        false
    }
}

macro_rules! impl_as_db_type {
    ($type:ty, $variant:ident) => {
        impl AsDbType for $type {
            fn as_db_type(_annotations: &[imr::Annotation]) -> imr::DbType {
                imr::DbType::$variant
            }
        }
    };
}
impl_as_db_type!(Vec<u8>, VarBinary);
impl_as_db_type!(i8, Int8);
impl_as_db_type!(i16, Int16);
impl_as_db_type!(i32, Int32);
impl_as_db_type!(i64, Int64);
impl_as_db_type!(isize, Int64);
impl_as_db_type!(u8, UInt8);
impl_as_db_type!(u16, UInt16);
impl_as_db_type!(u32, UInt32);
impl_as_db_type!(u64, UInt64);
impl_as_db_type!(usize, UInt64);
impl_as_db_type!(f32, Float);
impl_as_db_type!(f64, Double);
impl_as_db_type!(bool, Boolean);
impl AsDbType for String {
    fn as_db_type(annotations: &[imr::Annotation]) -> imr::DbType {
        let mut choices = false;
        for annotation in annotations.iter() {
            match annotation {
                imr::Annotation::Choices(_) => {
                    choices = true;
                }
                _ => {}
            }
        }
        if choices {
            imr::DbType::Choices
        } else {
            imr::DbType::VarChar
        }
    }
}
impl<T: AsDbType> AsDbType for Option<T> {
    fn as_db_type(annotations: &[imr::Annotation]) -> imr::DbType {
        T::as_db_type(annotations)
    }

    fn implicit_annotations() -> Vec<imr::Annotation> {
        T::implicit_annotations()
    }

    fn is_nullable() -> bool {
        true
    }
}

/// Map a rust enum, whose variant don't hold any data, into a database enum
///
/// ```rust
/// #[derive(rorm::DbEnum)]
/// pub enum Gender {
///     Male,
///     Female,
///     Other,
/// }
/// ```
pub trait DbEnum {
    /// Convert a string into its corresponding variant.
    ///
    /// # Panics
    /// Panics, if no variant matches. Since the string should only come from the db,
    /// a non matching string would indicate an invalid db state.
    fn from_str(string: &str) -> Self;

    /// Convert a variant into its corresponding string.
    fn to_str(&self) -> &'static str;

    /// Construct a vector containing all variants as strings.
    ///
    /// This will be called in order to construct the Intermediate Model Representation.
    fn as_choices() -> Vec<String>;
}
impl<E: DbEnum> AsDbType for E {
    fn as_db_type(_annotations: &[imr::Annotation]) -> imr::DbType {
        imr::DbType::Choices
    }

    fn implicit_annotations() -> Vec<imr::Annotation> {
        vec![imr::Annotation::Choices(E::as_choices())]
    }
}

/// A ModelDefinition provides methods to do something similar to reflection on model structs.
///
/// This trait is only implemented on empty types and used as dyn objects i.e. it is a higher
/// level representation for a function table.
/// It is automatically implemented for you by the [`derive(Model)`] attribute.
///
/// [`derive(Model)`]: crate::Model
// sync and send is required in order to store it as a static
pub trait GetModelDefinition: Sync + Send {
    /// Build rorm's model representation.
    fn as_rorm(&self) -> ModelDefinition;

    /// Build the Intermediate Model Representation
    fn as_imr(&self) -> imr::Model {
        self.as_rorm().into()
    }
}

/// Trait implementing most database interactions for a struct.
///
/// It should only ever be generated using [`derive(Model)`].
///
/// [`derive(Model)`]: crate::Model
pub trait Model {
    /// An enum generated by the `derive(Model)` macro listing all valid fields.
    /// It can be used to in macros to check at compile time
    /// that an arbitrary identifier is a valid field name:
    /// ```ignore
    /// let _ = <$model as ::rorm::model_def::Model>::Fields::$field;
    /// ```
    /// resulting in the following error message if it is not:
    /// ```plain
    /// no variant or associated item named `$field` found for enum `__$model_Fields` in the current scope
    /// ```
    type Fields;

    /// Returns the table name of the model
    fn table_name() -> &'static str;
}

/// rorm's model representation holding all data about a specific model.
///
/// This is very similar to the [Intermediate Model Representation](imr::Model). But it contains
/// more information and uses a slightly different format.
/// (For example using `&'static str` instead of `String`)
///
/// # WIP
/// This representations doesn't do much currently, but it is planned to be used in resolving relations.
pub struct ModelDefinition {
    /// Name of the table
    pub name: &'static str,

    /// Fields the Model has attached
    pub fields: Vec<Field>,

    /// Optional location of the source of this model
    pub source: Option<imr::Source>,
}

impl From<ModelDefinition> for imr::Model {
    fn from(model: ModelDefinition) -> Self {
        imr::Model {
            name: model.name.to_string(),
            fields: model.fields.into_iter().map(From::from).collect(),
            source_defined_at: model.source,
        }
    }
}

/// [`ModelDefinitions`]'s fields.
///
/// This is similar to [`imr::Field`]. See [`ModelDefinition`] for the why.
pub struct Field {
    /// Name of this field
    pub name: &'static str,

    /// [imr::DbType] of this field
    pub db_type: imr::DbType,

    /// List of annotations this field has set
    pub annotations: Vec<imr::Annotation>,

    /// Whether this field is nullable or not
    pub nullable: bool,

    /// Optional definition of the location of field in the source code
    pub source: Option<imr::Source>,
}

impl From<Field> for imr::Field {
    fn from(field: Field) -> Self {
        let mut annotations: Vec<_> = field.annotations.into_iter().map(From::from).collect();
        if !field.nullable {
            annotations.push(imr::Annotation::NotNull);
        }
        imr::Field {
            name: field.name.to_string(),
            db_type: field.db_type,
            annotations,
            source_defined_at: field.source,
        }
    }
}

/// The type to add to most models as primary key:
/// ```ignore
/// use rorm::{Model, ID};
///
/// #[derive(Model)]
/// struct SomeModel {
///     id: ID,
///     ..
/// }
pub type ID = GenericId<u64>;

/// Generic Wrapper which implies the primary key and autoincrement annotation
#[derive(Copy, Clone, Debug)]
pub struct GenericId<I: AsDbType>(pub I);

impl<I: AsDbType> AsDbType for GenericId<I> {
    fn as_db_type(annotations: &[imr::Annotation]) -> imr::DbType {
        I::as_db_type(annotations)
    }

    fn implicit_annotations() -> Vec<imr::Annotation> {
        let mut annotations = I::implicit_annotations();
        annotations.push(imr::Annotation::PrimaryKey); // TODO check if already
        annotations.push(imr::Annotation::AutoIncrement);
        annotations
    }
}

impl<I: AsDbType> From<I> for GenericId<I> {
    fn from(id: I) -> Self {
        GenericId(id)
    }
}

impl<I: AsDbType> Deref for GenericId<I> {
    type Target = I;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<I: AsDbType> DerefMut for GenericId<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
