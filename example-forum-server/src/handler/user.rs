use axum::extract::{Path, State};
use axum::Json;
use rorm::fields::types::MaxStr;
use rorm::{and, Database, FieldAccess};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::handler::{ApiError, ApiResult, SessionUser};
use crate::models::user::{NewUser, User, UserRole};

#[derive(Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: MaxStr<255>,
    pub password: String,
}
pub async fn register(
    State(db): State<Database>,
    session: Session,
    Json(request): Json<RegisterRequest>,
) -> ApiResult<()> {
    let mut tx = db.start_transaction().await?;
    if rorm::query(&mut tx, (User.id,))
        .condition(User.username.equals(&*request.username))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::BadRequest(format!(
            "The username `{}` is already taken",
            request.username
        )));
    }
    let id = rorm::insert(&mut tx, User)
        .return_primary_key()
        .single(&NewUser {
            username: request.username,
            password: request.password,
            role: UserRole::User,
        })
        .await?;
    session.insert("user_id", &id).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}
pub async fn login(
    State(db): State<Database>,
    session: Session,
    Json(request): Json<LoginRequest>,
) -> ApiResult<()> {
    if let Some((id,)) = rorm::query(&db, (User.id,))
        .condition(and![
            User.username.equals(&request.username),
            User.password.equals(&request.password)
        ])
        .optional()
        .await?
    {
        session.insert("user_id", &id).await?;
        Ok(())
    } else {
        Err(ApiError::BadRequest(
            "Invalid password or username".to_string(),
        ))
    }
}

pub async fn logout(session: Session) -> ApiResult<()> {
    session.flush().await?;
    Ok(())
}

pub async fn delete(
    State(db): State<Database>,
    SessionUser(user): SessionUser,
    session: Session,
) -> ApiResult<()> {
    rorm::delete(&db, User)
        .condition(User.id.equals(user.id))
        .await?;
    session.flush().await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ProfileResponse {
    pub username: String,
    pub role: String,
    pub posts: i64,
}
pub async fn profile(
    State(db): State<Database>,
    SessionUser(_user): SessionUser,
    Path(username): Path<String>,
) -> ApiResult<Json<ProfileResponse>> {
    let mut tx = db.start_transaction().await?;
    let (role,) = rorm::query(&mut tx, (User.role,))
        .condition(User.username.equals(&username))
        .optional()
        .await?
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown user: {username}")))?;
    let (posts,) = rorm::query(&mut tx, (User.posts.uuid.count(),))
        .condition(User.username.equals(&username))
        .one()
        .await?;
    tx.commit().await?;
    Ok(Json(ProfileResponse {
        username,
        role: role.to_string(),
        posts,
    }))
}
