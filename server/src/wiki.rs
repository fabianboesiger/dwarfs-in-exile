use askama::Template;
use crate::ServerError;

#[derive(Template, Default)]
#[template(path = "wiki.html")]
pub struct WikiTemplate {
}

pub async fn get_wiki(
) -> Result<WikiTemplate, ServerError> {
    Ok(WikiTemplate {})
}
