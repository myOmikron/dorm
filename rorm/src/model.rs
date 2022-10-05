use std::marker::PhantomData;

use rorm_db::conditional::{self, Condition};
use rorm_db::value::Value;
use rorm_declaration::hmr;
use rorm_declaration::imr;

/// This trait maps rust types to database types
pub trait AsDbType {
    /// A type which can be retrieved from the db and then converted into Self.
    type Primitive;

    /// The database type as defined in the Intermediate Model Representation
    type DbType: hmr::DbType;

    /// Convert the associated primitive type into `Self`.
    ///
    /// This function allows "non-primitive" types like [GenericId] or any [DbEnum] to implement
    /// their decoding without access to the underlying db details (namely `sqlx::Decode`)
    fn from_primitive(primitive: Self::Primitive) -> Self;

    /// Convert a reference to `Self` into the primitive [`Value`] used by our db implementation.
    fn as_primitive(&self) -> Value;

    /// Whether this type supports null.
    const IS_NULLABLE: bool = false;

    /// Extend a Vec with migrator annotations which are implied by the type.
    fn implicit_annotations(_annotations: &mut Vec<imr::Annotation>) {}
}

macro_rules! impl_as_db_type {
    ($type:ty, $db_type:ident, $value_variant:ident $(using $method:ident)?) => {
        impl AsDbType for $type {
            type Primitive = Self;

            type DbType = hmr::$db_type;

            #[inline(always)]
            fn from_primitive(primitive: Self::Primitive) -> Self {
                primitive
            }

            impl_as_db_type!(impl_as_primitive, $type, $db_type, $value_variant $(using $method)?);
        }
    };
    (impl_as_primitive, $type:ty, $db_type:ident, $value_variant:ident) => {
        #[inline(always)]
        fn as_primitive(&self) -> Value {
            Value::$value_variant(*self)
        }
    };
    (impl_as_primitive, $type:ty, $db_type:ident, $value_variant:ident using $method:ident) => {
        #[inline(always)]
        fn as_primitive(&self) -> Value {
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
    type Primitive = Self;
    type DbType = T::DbType;

    #[inline(always)]
    fn from_primitive(primitive: Self::Primitive) -> Self {
        primitive
    }

    fn as_primitive(&self) -> Value {
        match self {
            Some(value) => value.as_primitive(),
            None => Value::Null,
        }
    }

    const IS_NULLABLE: bool = true;

    fn implicit_annotations(annotations: &mut Vec<imr::Annotation>) {
        T::implicit_annotations(annotations)
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
    type Primitive = String;
    type DbType = hmr::Choices;

    fn from_primitive(primitive: Self::Primitive) -> Self {
        E::from_str(&primitive)
    }

    fn as_primitive(&self) -> Value {
        Value::String(self.to_str())
    }

    fn implicit_annotations(annotations: &mut Vec<imr::Annotation>) {
        annotations.push(imr::Annotation::Choices(E::as_choices()));
    }
}

/// Trait implemented on Patches i.e. a subset of a model's fields.
///
/// Implemented by [`derive(Patch)`] as well as [`derive(Model)`].
pub trait Patch {
    /// The model this patch is for
    type Model: Model;

    /// List of columns i.e. fields this patch contains
    const COLUMNS: &'static [&'static str];

    /// List of fields' indexes this patch contains
    ///
    /// Used in [`contains_index`]
    const INDEXES: &'static [usize];

    /// Get a field's db value by its index
    fn get(&self, index: usize) -> Option<Value>;

    /// Build a [Condition] which only matches on this instance.
    ///
    /// This method defaults to using the primary key.
    /// If the patch does not store the models primary key, this method will return `None`.
    fn as_condition(&self) -> Option<Condition> {
        self.get(Self::Model::PRIMARY.1).map(|value| {
            Condition::BinaryCondition(conditional::BinaryCondition::Equals(Box::new([
                Condition::Value(Value::Ident(Self::Model::PRIMARY.0)),
                Condition::Value(value),
            ])))
        })
    }
}

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
    P::INDEXES.iter().map(|&index| patch.get(index)).flatten()
}

/// Trait implementing most database interactions for a struct.
///
/// It should only ever be generated using [`derive(Model)`].
///
/// [`derive(Model)`]: crate::Model
pub trait Model: Patch<Model = Self> {
    /// The primary key's name and index
    const PRIMARY: (&'static str, usize);

    /// A struct which "maps" field identifiers their descriptions (i.e. [`Field<T>`]).
    ///
    /// The struct is constructed once in the [`Model::FIELDS`] constant.
    type Fields;

    /// A constant struct which "maps" field identifiers their descriptions (i.e. [`Field<T>`]).
    // Actually FIELDS is an alias for F instead of the other way around.
    // This changes was made in the hope it would improve IDE support.
    const FIELDS: Self::Fields = Self::F;

    /// Shorthand version of [`FIELDS`]
    ///
    /// [`FIELDS`]: Model::FIELDS
    const F: Self::Fields;

    /// Returns the table name of the model
    fn table_name() -> &'static str;

    /// Returns the model's intermediate representation
    ///
    /// As library user you probably won't need this. You might want to look at [`write_models`].
    ///
    /// [`write_models`]: crate::write_models
    fn get_imr() -> imr::Model;
}

/// All relevant information about a model's field
#[derive(Copy, Clone)]
pub struct Field<T: 'static, D: hmr::DbType> {
    /// This field's position in the model.
    pub index: usize,

    /// Name of this field
    pub name: &'static str,

    /// List of annotations this field has set
    pub annotations: &'static [Annotation],

    /// Optional definition of the location of field in the source code
    pub source: Option<Source>,

    #[doc(hidden)]
    pub _phantom: PhantomData<&'static (T, D)>,
}

impl<T: AsDbType, D: hmr::DbType> Field<T, D> {
    /// Reexport [`AsDbType::from_primitive`]
    ///
    /// This method makes macros' syntax slightly cleaner
    #[inline(always)]
    pub fn convert_primitive(&self, primitive: T::Primitive) -> T {
        T::from_primitive(primitive)
    }
}

impl<T: AsDbType, D: hmr::DbType> From<&'_ Field<T, D>> for imr::Field {
    fn from(field: &'_ Field<T, D>) -> Self {
        let mut annotations: Vec<_> = field.annotations.iter().map(|&anno| anno.into()).collect();
        T::implicit_annotations(&mut annotations);
        if !T::IS_NULLABLE {
            annotations.push(imr::Annotation::NotNull);
        }
        imr::Field {
            name: field.name.to_string(),
            db_type: D::IMR,
            annotations,
            source_defined_at: field.source.map(Into::into),
        }
    }
}

/// Location in the source code a [Model] or [Field] originates from
/// Used for better error messages in the migration tool
#[derive(Copy, Clone)]
pub struct Source {
    /// Filename of the source code of the [Model] or [Field]
    pub file: &'static str,
    /// Line of the [Model] or [Field]
    pub line: usize,
    /// Column of the [Model] or [Field]
    pub column: usize,
}

impl From<Source> for imr::Source {
    fn from(source: Source) -> Self {
        imr::Source {
            file: source.file.to_string(),
            line: source.line,
            column: source.column,
        }
    }
}

/// The subset of annotations which need to be communicated with the migration tool
#[derive(Copy, Clone)]
pub enum Annotation {
    /// Only for [imr::DbType::Timestamp], [imr::DbType::Datetime], [imr::DbType::Time] and [imr::DbType::Date].
    /// Will set the current time of the database when a row is created.
    AutoCreateTime,
    /// Only for [imr::DbType::Timestamp], [imr::DbType::Datetime], [imr::DbType::Time] and [imr::DbType::Date].
    /// Will set the current time of the database when a row is updated.
    AutoUpdateTime,
    /// AUTO_INCREMENT constraint
    AutoIncrement,
    /// A list of choices to set
    Choices(&'static [&'static str]),
    /// DEFAULT constraint
    DefaultValue(DefaultValue),
    /// Create an index. The optional [imr::IndexValue] can be used, to build more complex indexes.
    Index(Option<IndexValue>),
    /// Only for VARCHAR. Specifies the maximum length of the column's content.
    MaxLength(i32),
    /// NOT NULL constraint
    NotNull,
    /// The annotated column will be used as primary key
    PrimaryKey,
    /// UNIQUE constraint
    Unique,
}

impl From<Annotation> for imr::Annotation {
    fn from(anno: Annotation) -> Self {
        match anno {
            Annotation::AutoCreateTime => imr::Annotation::AutoCreateTime,
            Annotation::AutoUpdateTime => imr::Annotation::AutoUpdateTime,
            Annotation::AutoIncrement => imr::Annotation::AutoIncrement,
            Annotation::Choices(choices) => {
                imr::Annotation::Choices(choices.into_iter().map(ToString::to_string).collect())
            }
            Annotation::DefaultValue(value) => imr::Annotation::DefaultValue(match value {
                DefaultValue::String(string) => imr::DefaultValue::String(string.to_string()),
                DefaultValue::Boolean(boolean) => imr::DefaultValue::Boolean(boolean),
                DefaultValue::Float(float) => imr::DefaultValue::Float(float.into()),
                DefaultValue::Integer(integer) => imr::DefaultValue::Integer(integer),
            }),
            Annotation::Index(index) => {
                imr::Annotation::Index(index.map(|index| imr::IndexValue {
                    name: index.name.to_string(),
                    priority: index.priority,
                }))
            }
            Annotation::MaxLength(length) => imr::Annotation::MaxLength(length),
            Annotation::NotNull => imr::Annotation::NotNull,
            Annotation::PrimaryKey => imr::Annotation::PrimaryKey,
            Annotation::Unique => imr::Annotation::Unique,
        }
    }
}

/// Represents a complex index
#[derive(Copy, Clone)]
pub struct IndexValue {
    /// Name of the index. Can be used multiple times in a [Model] to create an
    /// index with multiple columns.
    pub name: &'static str,

    /// The order to put the columns in while generating an index.
    /// Only useful if multiple columns with the same name are present.
    pub priority: Option<i32>,
}

/// A column's default value which is any non object / array json value
#[derive(Copy, Clone)]
pub enum DefaultValue {
    /// Use hexadecimal to represent binary data
    String(&'static str),
    /// i128 is used as it can represent any integer defined in DbType
    Integer(i128),
    /// Ordered float is used as f64 does not Eq and Order which are needed for Hash
    Float(f64),
    /// Just a bool. Nothing interesting here.
    Boolean(bool),
}
