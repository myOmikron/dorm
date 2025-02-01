use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::Json;
use futures_util::TryStreamExt;
use rorm::fields::types::MaxStr;
use rorm::prelude::ForeignModelByField;
use rorm::{and, Database, FieldAccess};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::handler::{ApiError, ApiResult, SessionUser};
use crate::models::post::{NewPost, Post};
use crate::models::thread::{NewThread, Thread};
use crate::models::user::User;

#[derive(Serialize, Deserialize)]
pub struct CreateThreadRequest {
    pub name: String,
}

pub async fn create(
    State(db): State<Database>,
    SessionUser(_user): SessionUser,
    Json(request): Json<CreateThreadRequest>,
) -> ApiResult<Json<String>> {
    let mut tx = db.start_transaction().await?;
    let identifier = request.name.to_ascii_lowercase();
    if rorm::query(&mut tx, Thread)
        .condition(Thread.identifier.equals(&identifier))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::BadRequest(
            "Please choose another name".to_string(),
        ));
    }
    rorm::insert(&mut tx, Thread)
        .single(&NewThread {
            identifier: identifier.clone(),
            name: request.name,
        })
        .await?;
    tx.commit().await?;
    Ok(Json(identifier))
}

#[derive(Serialize, Deserialize)]
pub struct GetResponse {
    pub identifier: String,
    pub name: String,
    pub opened_at: OffsetDateTime,
    pub posts: Vec<ThreadPost>,
}
#[derive(Serialize, Deserialize)]
pub struct ThreadPost {
    pub uuid: Uuid,
    pub user: Option<MaxStr<255>>,
    pub message: String,
    pub posted_at: OffsetDateTime,
    pub replies: i64,
}
pub async fn get(
    State(db): State<Database>,
    Path(thread): Path<String>,
) -> ApiResult<Json<GetResponse>> {
    let mut tx = db.start_transaction().await?;

    let (name, opened_at) = rorm::query(&mut tx, (Thread.name, Thread.opened_at))
        .condition(Thread.identifier.equals(&thread))
        .optional()
        .await?
        .ok_or_else(|| ApiError::BadRequest("Unknown thread".to_ascii_lowercase()))?;

    let users = rorm::query(&mut tx, (User.id, User.username))
        .stream()
        .try_collect::<HashMap<_, _>>()
        .await?;

    let posts: Vec<_> = rorm::query(
        &mut tx,
        (Post.uuid, Post.message, Post.user, Post.posted_at),
    )
    .condition(Post.thread.equals(&thread))
    .order_asc(Post.posted_at)
    .stream()
    .map_ok(|(uuid, message, user, posted_at)| ThreadPost {
        uuid,
        user: user.map(|ForeignModelByField(id)| users[&id].clone()),
        message: message.into_inner(),
        posted_at,
        replies: 0,
    })
    .try_collect()
    .await?;

    tx.commit().await?;
    Ok(Json(GetResponse {
        identifier: thread,
        name,
        opened_at,
        posts,
    }))
}

#[derive(Serialize, Deserialize)]
pub struct MakePostRequest {
    pub message: String,
    pub reply_to: Option<Uuid>,
}
pub async fn make_post(
    State(db): State<Database>,
    SessionUser(user): SessionUser,
    Path(thread): Path<String>,
    Json(request): Json<MakePostRequest>,
) -> ApiResult<()> {
    let mut tx = db.start_transaction().await?;

    rorm::query(&mut tx, (Thread.identifier,))
        .condition(Thread.identifier.equals(&thread))
        .optional()
        .await?
        .ok_or_else(|| ApiError::BadRequest("Unknown thread".to_string()))?;

    if let Some(reply_to) = request.reply_to {
        rorm::query(&mut tx, (Post.uuid,))
            .condition(and![
                Post.uuid.equals(reply_to),
                Post.thread.equals(&thread),
            ])
            .optional()
            .await?
            .ok_or_else(|| ApiError::BadRequest("Unknown post".to_string()))?;
    }

    rorm::insert(&mut tx, Post)
        .return_nothing()
        .single(&NewPost {
            uuid: Uuid::new_v4(),
            message: MaxStr::new(request.message)
                .map_err(|_| ApiError::BadRequest("Post's message is too long".to_string()))?,
            user: Some(ForeignModelByField(user.id)),
            thread: ForeignModelByField(thread),
            reply_to: request.reply_to.map(ForeignModelByField),
        })
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn delete(
    State(db): State<Database>,
    SessionUser(_user): SessionUser,
    Path(identifier): Path<String>,
) -> ApiResult<()> {
    rorm::delete(&db, Thread)
        .condition(Thread.identifier.equals(&identifier))
        .await?;
    Ok(())
}
