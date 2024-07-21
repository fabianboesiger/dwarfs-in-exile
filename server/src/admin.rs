use crate::{game::GameState, ServerError};
use askama::Template;
use askama_axum::Response;
use axum::{
    response::IntoResponse,
    Extension, Form,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;
use bcrypt::hash;

#[derive(Debug, Deserialize)]
pub struct ManageUser {
    user_id: i64,
    password: Option<String>,
    add_premium: Option<i64>,
    delete: Option<bool>,
}

#[derive(Template, Default)]
#[template(path = "admin.html")]
pub struct AdminTemplate {
    users: Vec<(i64, String)>,
}

pub async fn get_admin(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
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
    
    let users: Vec<(i64, String)> = sqlx::query_as(
        r#"
                SELECT user_id, username
                FROM users
            "#,
    )
    .bind(&user_id)
    .fetch_all(&pool)
    .await?;


    Ok(AdminTemplate {
        users
    }.into_response())
}

pub async fn post_manage_user(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
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
    
    let mut tx = pool.begin().await?;

    if manage_user.delete.unwrap_or(false) {
        sqlx::query(
            r#"
                    DELETE FROM users
                    WHERE user_id = $1
                "#,
        )
        .bind(&manage_user.user_id)
        .execute(&mut tx)
        .await?;
    } else {
        if let Some(password) = manage_user.password {
            if !password.is_empty() {
                let hashed = tokio::task::spawn_blocking(move || hash(&password, 4).unwrap())
                    .await
                    .unwrap();

                sqlx::query(
                    r#"
                            UPDATE users
                            SET password = $2
                            WHERE user_id = $1
                        "#,
                )
                .bind(&manage_user.user_id)
                .bind(&hashed)
                .execute(&mut tx)
                .await?;
            }
        }

        if let Some(add_premium) = manage_user.add_premium {
            if add_premium > 0 {
                sqlx::query(
                    r#"
                            UPDATE users
                            SET premium = premium + $2
                            WHERE user_id = $1
                        "#,
                )
                .bind(&manage_user.user_id)
                .bind(&add_premium)
                .execute(&mut tx)
                .await?;
            }
        }
    }

    tx.commit().await?;

    game_state.new_server_connection().await.updated_user_data();

    Ok("ok".into_response())
}


pub async fn post_create_world(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
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

    game_state.create().await?;

    Ok("ok".into_response())
}
