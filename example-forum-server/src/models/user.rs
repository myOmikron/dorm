use std::fmt;
use std::str::FromStr;

use rorm::fields::types::MaxStr;
use rorm::prelude::*;

use crate::models::post::Post;

#[derive(Model)]
pub struct User {
    /// This is an auto-increment which should not be "leaked" to the user
    #[rorm(id)]
    pub id: i64,

    /// The user's unique identifier
    #[rorm(unique)]
    pub username: MaxStr<255>,

    /// Let's store passwords in plain text using a max length of 16.
    ///
    /// Everyone will love us for it <3
    #[rorm(max_length = 16)]
    pub password: String,

    /// What are the user's permissions and responsibilities?
    pub role: UserRole,

    pub posts: BackRef<field!(Post.user)>,
}

#[derive(Patch)]
#[rorm(model = "User")]
pub struct NewUser {
    pub username: MaxStr<255>,
    pub password: String,
    pub role: UserRole,
}

#[derive(DbEnum)]
pub enum UserRole {
    User,
    Moderator,
    Admin,
}
impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::User => "user",
            Self::Moderator => "moderator",
            Self::Admin => "admin",
        })
    }
}
impl FromStr for UserRole {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "user" => Self::User,
            "moderator" => Self::Moderator,
            "admin" => Self::Admin,
            _ => return Err(()),
        })
    }
}
