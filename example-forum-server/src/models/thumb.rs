use rorm::prelude::*;

use crate::models::post::Post;
use crate::models::user::User;

#[derive(Model)]
pub struct Thumb {
    /// Some internal id
    #[rorm(id)]
    pub id: i64,

    /// Is the thumb pointing up?
    pub is_up: bool,

    /// The user who gave the thumb
    #[rorm(on_delete = "SetNull")]
    pub user: Option<ForeignModel<User>>,

    /// The post which received the thumb
    #[rorm(on_delete = "Cascade")]
    pub post: ForeignModel<Post>,
}

#[derive(Patch)]
#[rorm(model = "Thumb")]
pub struct NewThumb {
    pub is_up: bool,
    pub user: Option<ForeignModel<User>>,
    pub post: ForeignModel<Post>,
}
