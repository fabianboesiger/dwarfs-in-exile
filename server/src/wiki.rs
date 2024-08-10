use crate::ServerError;
use askama::Template;

#[derive(Template, Default)]
#[template(path = "wiki.html")]
pub struct WikiTemplate {}

pub async fn get_wiki() -> Result<WikiTemplate, ServerError> {
    Ok(WikiTemplate {})
}
