use crate::{game::GameState, ServerError};
use askama::Template;
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension, Form,
};
use bcrypt::hash;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct ManageUser {
    user_id: i64,
    password: Option<String>,
    add_premium: Option<i64>,
    delete: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    free_premium: i64,
}

#[derive(Debug, Deserialize, Default)]
struct User {
    user_id: i64,
    username: String,
    premium: i64,
}

#[derive(Debug, Deserialize, Default)]
struct Game {
    id: i64,
    winner: Option<i64>,
}

#[derive(Template, Default)]
#[template(path = "admin.html")]
pub struct AdminTemplate {
    settings: Settings,
    users: Vec<User>,
    games: Vec<Game>,
}

#[derive(Debug, Deserialize)]
pub struct AddPremium {
    add_premium: i64,
}

pub async fn get_admin(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

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

    let (free_premium,): (i64,) = sqlx::query_as(
        r#"
                SELECT free_premium
                FROM settings
                LIMIT 1
            "#,
    )
    .fetch_one(&pool)
    .await?;

    let settings = Settings { free_premium };

    let users = sqlx::query_as(
        r#"
                SELECT user_id, username, premium
                FROM users
            "#,
    )
    .bind(&user_id)
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|(user_id, username, premium)| User {
        user_id,
        username,
        premium,
    })
    .collect();

    let games = sqlx::query_as(
        r#"
                SELECT id, winner
                FROM games
            "#,
    )
    .bind(&user_id)
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|(id, winner)| Game { id, winner })
    .collect();

    Ok(AdminTemplate {
        users,
        settings,
        games,
    }
    .into_response())
}

pub async fn post_manage_user(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    Form(manage_user): Form<ManageUser>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

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

    Ok(Redirect::to("/admin").into_response())
}


pub async fn post_add_premium(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    Form(add_premium): Form<AddPremium>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

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

    if add_premium.add_premium > 0 {
        sqlx::query(
            r#"
                    UPDATE users
                    SET premium = premium + $1
                "#,
        )
        .bind(&add_premium.add_premium)
        .execute(&pool)
        .await?;
    }

    game_state.new_server_connection().await.updated_user_data();

    Ok(Redirect::to("/admin").into_response())
}


pub async fn post_create_world(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

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

    Ok(Redirect::to("/admin").into_response())
}

pub async fn post_update_settings(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Form(settings): Form<Settings>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

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

    sqlx::query(
        r#"
                    UPDATE settings
                    SET free_premium = $1
                "#,
    )
    .bind(&settings.free_premium)
    .execute(&pool)
    .await?;

    Ok(Redirect::to("/admin").into_response())
}
