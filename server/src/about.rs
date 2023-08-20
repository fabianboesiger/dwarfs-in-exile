use askama::Template;

#[derive(Template, Default)]
#[template(path = "about.html")]
pub struct AboutTemplate {}

pub async fn get_about() -> AboutTemplate {
    AboutTemplate::default()
}
