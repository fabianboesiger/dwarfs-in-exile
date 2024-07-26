use askama::Template;
use axum::Extension;
use sqlx::SqlitePool;

use crate::ServerError;

#[derive(Template, Default)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    free_premium: i64
}

pub async fn get_index(
    Extension(pool): Extension<SqlitePool>,
) -> Result<IndexTemplate, ServerError> {
    let (free_premium,): (i64,) = sqlx::query_as(
        r#"
                SELECT free_premium
                FROM settings
                LIMIT 1
            "#,
    )
    .fetch_one(&pool)
    .await?;

    Ok(IndexTemplate { free_premium })
}
