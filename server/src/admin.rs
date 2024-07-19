use crate::ServerError;
use askama::{DynTemplate, Template};
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension, Form,
};
use bcrypt::verify;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct ManageUser {
    username: String,
    password: Option<String>,
    add_premium: i64,
    delete: bool,
}

#[derive(Template, Default)]
#[template(path = "admin.html")]
pub struct AdminTemplate {
}

pub async fn get_admin(
) -> AdminTemplate {
    AdminTemplate::default()
}

pub async fn post_manage_user(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Form(manage_user): Form<ManageUser>,
) -> Result<Response, ServerError> {
    let user_id = session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?;

    let result: (i64,) = sqlx::query_as(
        r#"
                SELECT admin
                FROM users
                WHERE user_id = $1
            "#,
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await?;

    let admin = result.0 == 1;

    if !admin {
        return Err(ServerError::NoAdminPermissions);
    }
    


    //if admin.
    Ok("test".into_response())
}
