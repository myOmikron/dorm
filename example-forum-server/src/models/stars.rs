use rorm::conditions::Value;
use rorm::db::sql::value::NullType;
use rorm::fields::traits::{Array, FieldColumns, FieldType};
use rorm::fields::utils::check::shared_linter_check;
use rorm::fields::utils::get_annotations::forward_annotations;
use rorm::fields::utils::get_names::single_column_name;
use rorm::prelude::ForeignModel;
use rorm::{Model, Patch};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::post::Post;
use crate::models::user::User;

#[derive(Model)]
pub struct Stars {
    /// Some internal id
    #[rorm(id)]
    pub id: i64,

    /// The number of stars given
    pub amount: StarsAmount,

    /// The user who gave the stars
    #[rorm(on_delete = "SetNull")]
    pub user: Option<ForeignModel<User>>,

    /// The post which received the stars
    #[rorm(on_delete = "Cascade")]
    pub post: ForeignModel<Post>,
}

#[derive(Patch)]
#[rorm(model = "Stars")]
pub struct NewStars {
    /// The number of stars given
    pub amount: StarsAmount,

    /// The user who gave the stars
    pub user: Option<ForeignModel<User>>,

    /// The post which received the stars
    pub post: ForeignModel<Post>,
}

/// Newtype to represent the number of stars a user gave a post
///
/// It ranges from 0 to 5 (inclusive).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
pub struct StarsAmount(i16);
impl StarsAmount {
    pub fn new(value: i16) -> Option<Self> {
        (0..=5).contains(&value).then_some(Self(value))
    }
    pub fn get(self) -> i16 {
        self.0
    }
}
impl<'de> Deserialize<'de> for StarsAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Unexpected};
        i16::deserialize(deserializer).and_then(|value| {
            Self::new(value).ok_or(Error::invalid_value(
                Unexpected::Signed(value as i64),
                &"a number from 0 to 5",
            ))
        })
    }
}
impl FieldType for StarsAmount {
    type Columns = Array<1>;

    const NULL: FieldColumns<Self, NullType> = [NullType::I16];

    fn into_values<'a>(self) -> FieldColumns<Self, Value<'a>> {
        self.0.into_values()
    }

    fn as_values(&self) -> FieldColumns<Self, Value<'_>> {
        self.0.as_values()
    }

    type Decoder = StarsAmountDecoder;
    type GetNames = single_column_name;
    type GetAnnotations = forward_annotations<1>;
    type Check = shared_linter_check<1>;
}
rorm::new_converting_decoder! {
    pub StarsAmountDecoder,
    |value: i16| -> StarsAmount {
        StarsAmount::new(value).ok_or_else(
            || format!("Got invalid number of stars: {value}")
        )
    }
}
