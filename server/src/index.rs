use askama::Template;

#[derive(Template, Default)]
#[template(path = "index.html")]
pub struct IndexTemplate {}

pub async fn get_index() -> IndexTemplate {
    IndexTemplate::default()
}
