use crate::session_theme::Theme;
use htmx_macro::hx_put;
use pavex::{Response, http::HeaderValue, request::body::UrlEncodedBody};
use pavex_session::Session;

#[derive(Debug, serde::Deserialize)]
pub struct ThemeForm {
    pub theme: String,
}

#[hx_put(path = "/theme")]
pub async fn set_theme_page(
    body: UrlEncodedBody<ThemeForm>,
    session: &mut Session<'_>,
) -> Response {
    let theme = Theme::from_str(&body.0.theme);
    session.insert("theme", theme.as_str().to_string()).await.ok();

    Response::ok()
        .set_typed_body("")
        .insert_header(
            pavex::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
}