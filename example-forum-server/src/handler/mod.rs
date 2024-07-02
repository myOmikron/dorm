mod thread;
mod user;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::Router;
use rorm::{query, Database, FieldAccess, Model};
use tower_sessions::{session, Session};

use crate::models::user::User;

pub fn get_router() -> Router<Database> {
    Router::new().nest(
        "/api",
        Router::new()
            .nest(
                "/user",
                Router::new()
                    .route("/register", post(user::register))
                    .route("/login", post(user::login))
                    .route("/logout", put(user::logout))
                    .route("/delete", put(user::delete))
                    .route("/profile/:username", get(user::profile)),
            )
            .nest(
                "/thread",
                Router::new()
                    .route("/create", post(thread::create))
                    .route("/get/:identifier", get(thread::get))
                    .route("/posts/:identifier", post(thread::make_post))
                    .route("/delete/:identifier", delete(thread::delete)),
            ),
    )
}

pub struct SessionUser(User);
#[axum::async_trait]
impl FromRequestParts<Database> for SessionUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, db: &Database) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, db)
            .await
            .map_err(|(_, msg)| ApiError::ServerError(format!("Session error: {msg}")))?;
        let user = query!(db, User)
            .condition(
                User::F.id.equals(
                    session
                        .get::<i64>("user_id")
                        .await?
                        .ok_or_else(|| ApiError::BadRequest("Please login first".to_string()))?,
                ),
            )
            .one()
            .await?;
        Ok(Self(user))
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub enum ApiError {
    BadRequest(String),
    ServerError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::BadRequest(body) => (StatusCode::BAD_REQUEST, body),
            ApiError::ServerError(body) => (StatusCode::INTERNAL_SERVER_ERROR, body),
        };
        let mut response = Response::new(body.into());
        *response.status_mut() = status;
        response
    }
}

impl From<rorm::Error> for ApiError {
    fn from(value: rorm::Error) -> Self {
        ApiError::ServerError(format!("Database error: {value}"))
    }
}
impl From<session::Error> for ApiError {
    fn from(value: session::Error) -> Self {
        ApiError::ServerError(format!("Session error: {value}"))
    }
}
