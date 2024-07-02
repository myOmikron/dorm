use rorm::fields::types::MaxStr;
use rorm::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::models::thread::Thread;
use crate::models::user::User;

#[derive(Model)]
pub struct Post {
    /// An uuid identifying the post
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The post's message
    pub message: MaxStr<1024>,

    /// The user how posted this post
    #[rorm(on_delete = "SetNull")]
    pub user: Option<ForeignModel<User>>,

    /// The thread this post was posted in
    #[rorm(on_delete = "Cascade")]
    pub thread: ForeignModel<Thread>,

    /// The post this one is a reply to if it is a reply at all
    #[rorm(on_delete = "Cascade")]
    pub reply_to: Option<ForeignModel<Post>>,

    /// When was this post posted?
    #[rorm(auto_create_time)]
    pub posted_at: OffsetDateTime,
}

#[derive(Patch)]
#[rorm(model = "Post")]
pub struct NewPost {
    pub uuid: Uuid,
    pub message: MaxStr<1024>,
    pub user: Option<ForeignModel<User>>,
    pub thread: ForeignModel<Thread>,
    pub reply_to: Option<ForeignModel<Post>>,
}
