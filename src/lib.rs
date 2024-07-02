//! Rorm is a rust orm.
//!
//! [List of all types valid as model fields](fields)
#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_auto_cfg))]
#![warn(missing_docs)]

#[cfg(all(feature = "all-drivers", feature = "postgres-only"))]
compile_error!("You cannot enable postgres-only with other drivers active");

pub use rorm_db::{Database, DatabaseConfiguration, DatabaseDriver, Error, Row};

pub use crate::internal::field::access::FieldAccess;
pub use crate::model::{Model, Patch};

/// Re-export of [rorm-cli](rorm_cli)
#[cfg(feature = "cli")]
pub mod cli {
    pub use rorm_cli::*;
}
/// Re-export of [rorm-db](rorm_db)
pub mod db {
    pub use rorm_db::*;
}
#[doc(hidden)] // used by macros
pub use linkme;
/// Re-exported for use in parser structs of user
pub use rorm_declaration::config;
#[doc(hidden)] // used by macros
pub use rorm_declaration::imr;

/// A prelude of common types, traits and derive macros that are used by `rorm`
pub mod prelude {
    pub use rorm_macro::{DbEnum, Model, Patch};

    pub use crate::field;
    pub use crate::fields::types::{BackRef, ForeignModel, ForeignModelByField};
    pub use crate::internal::field::access::FieldAccess;
    pub use crate::model::{Model, Patch};
}

pub use crate::crud::delete::delete;
pub use crate::crud::insert::insert;
pub use crate::crud::query::query;
pub use crate::crud::update::update;

pub mod conditions;
pub mod crud;
pub mod fields;
pub mod internal;
pub mod model;

/// This slice is populated by the [`Model`] macro with all models.
///
/// [`Model`]: rorm_macro::Model
#[allow(non_camel_case_types)]
#[linkme::distributed_slice]
#[doc(hidden)]
pub static MODELS: [fn() -> imr::Model] = [..];

/// Write all models in the Intermediate Model Representation to a [writer](std::io::Write).
pub fn write_models(writer: &mut impl std::io::Write) -> Result<(), serde_json::Error> {
    let imf = imr::InternalModelFormat {
        models: MODELS.iter().map(|func| func()).collect(),
    };
    serde_json::to_writer(writer, &imf)
}

/// Prints all models in the Intermediate Model Representation to stdout.
/// This should be used as a main function to produce the file for the migrator.
///
/// See also [`rorm_main`]
pub fn print_models() -> Result<(), serde_json::Error> {
    write_models(&mut std::io::stdout())
}

#[doc(hidden)]
pub(crate) mod private {
    pub trait Private {}
    impl Private for () {}
}
/// Put this macro inside a trait to seal it i.e. prevent extern implementations.
#[macro_export]
macro_rules! sealed {
    (trait) => {
        /// This method prohibits implementation of this trait out side of its defining crate.
        fn _not_implementable<Private: $crate::private::Private>();
    };
    (impl) => {
        fn _not_implementable<Private: $crate::private::Private>() {}
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! get_field {
    ($patch:ty, $field:ident) => {
        <<$patch as $crate::model::Patch>::Model as $crate::model::FieldByIndex<
            {
                $crate::internal::field::FieldProxy::index(|| {
                    <<Self as $crate::model::Patch>::Model as $crate::model::Model>::FIELDS.$field
                })
            },
        >>::Field
    };
}

/// Get the type for a model's field
///
/// Use this macro for generic parameter in [`ForeignModelByField`](fields::types::ForeignModelByField) and [`BackRef`](fields::types::BackRef).
#[macro_export]
macro_rules! field {
    ($model:ident.$field:ident) => {
        <$model as $crate::model::FieldByIndex<
            { $crate::internal::field::FieldProxy::index(|| $model.$field) },
        >>::Field
    };
    ($model:ident::F.$field:ident) => {
        <$model as $crate::model::FieldByIndex<
            { $crate::internal::field::FieldProxy::index(|| $model::F.$field) },
        >>::Field
    };
}

/// This attribute is put on your main function.
///
/// When you build with the `rorm-main` feature enabled this attribute will replace your main function.
/// The new main function will simply write all your defined models to `./.models.json`
/// to be further process by the migrator.
///
/// Make sure you have added the feature `rorm-main` to your crate i.e. put the following in your `Cargo.toml`:
/// ```toml
/// [features]
/// rorm-main = []
/// ```
///
/// If you don't like this feature name you can pass the attribute any other name to use instead:
/// ```
/// use rorm::rorm_main;
///
/// #[rorm_main("other-name")]
/// fn main() {}
/// ```
#[deprecated(note = "Use `write_models` or `print_models` to implement it yourself")]
pub use rorm_macro::rorm_main;
/// ```no_run
/// use rorm::DbEnum;
///
/// #[derive(DbEnum)]
/// pub enum Gender {
///     Male,
///     Female,
///     Other,
/// }
/// ```
pub use rorm_macro::DbEnum;
/// ```no_run
/// use rorm::Model;
///
/// #[derive(Model)]
/// struct User {
///
///     #[rorm(id)]
///     id: i32,
///
///     #[rorm(max_length = 255, unique)]
///     username: String,
///
///     #[rorm(max_length = 255)]
///     password: String,
///
///     #[rorm(default = false)]
///     admin: bool,
///
///     age: i16,
/// }
/// ```
pub use rorm_macro::Model;
/// ```no_run
/// use rorm::{Model, Patch};
///
/// #[derive(Model)]
/// struct User {
///
///     #[rorm(id)]
///     id: i32,
///
///     #[rorm(max_length = 255, unique)]
///     username: String,
///
///     #[rorm(max_length = 255)]
///     password: String,
///
///     #[rorm(default = false)]
///     admin: bool,
///
///     age: i16,
/// }
///
/// #[derive(Patch)]
/// #[rorm(model = "User")]
/// struct InsertNormalUser {
///     // id set by database
///
///     username: String,
///
///     password: String,
///
///     // admin defaults to false
///
///     age: i16,
/// }
/// ```
pub use rorm_macro::Patch;
