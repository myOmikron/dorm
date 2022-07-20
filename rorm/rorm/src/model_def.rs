use rorm_common::imr;

/// This slice is populated by the [`Model`] macro with all models.
///
/// [`Model`]: rorm_macro::Model
#[allow(non_camel_case_types)]
#[linkme::distributed_slice]
pub static MODELS: [&'static dyn GetModelDefinition] = [..];

/// A ModelDefinition provides methods to do something similar to reflection on model structs.
///
/// This trait is only implemented on empty types and used as dyn objects i.e. it is a higher
/// level representation for a function table.
/// It is automatically implemented for you by the [`derive(Model)`] attribute.
///
/// [`derive(Model)`]: crate::Model
// sync and send is required in order to store it as a static
pub trait GetModelDefinition: Sync + Send {
    fn as_rorm(&self) -> ModelDefinition;

    /// Build the Intermediate Model Representation
    fn as_imr(&self) -> imr::Model {
        self.as_rorm().into()
    }
}

/// Trait implementing most database iteractions for a struct.
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
}

pub struct ModelDefinition {
    pub name: &'static str,
    pub fields: Vec<Field>,
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

pub struct Field {
    pub name: &'static str,
    pub db_type: imr::DbType,
    pub annotations: Vec<imr::Annotation>,
    pub nullable: bool,
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
