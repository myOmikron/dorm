use rorm::prelude::*;
use time::OffsetDateTime;

use crate::models::post::Post;

#[derive(Model)]
pub struct Thread {
    /// Let's use this normalized version of `name` as primary key
    #[rorm(primary_key, max_length = 255)]
    pub identifier: String,

    /// The thread's display name
    #[rorm(max_length = 255)]
    pub name: String,

    /// When was this thread opened?
    #[rorm(auto_create_time)]
    pub opened_at: OffsetDateTime,

    pub posts: BackRef<field!(Post.thread)>,
}

#[derive(Patch)]
#[rorm(model = "Thread")]
pub struct NewThread {
    pub identifier: String,
    pub name: String,
}
