use crate::routes::api::users::password::validate_credentials;
use askama::Template;
use htmx_macro::{hx_get, hx_post};
use pavex::{Response, http::HeaderValue, request::body::UrlEncodedBody};
use pavex_session::Session;
use secrecy::Secret;
use sqlx::SqlitePool;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

#[hx_get(path = "/login", template = "login.html")]
pub fn login_page() -> Response {
    let html = LoginPage.render().expect("template render failed");
    html_response(html)
}

#[derive(serde::Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: Secret<String>,
}

#[derive(Template)]
#[template(path = "login_result.html")]
struct LoginResult {
    success: bool,
    error_message: String,
}

#[hx_post(path = "/login", template = "login_result.html")]
pub async fn login_submit(
    body: UrlEncodedBody<LoginForm>,
    db_pool: &SqlitePool,
    session: &mut Session<'_>,
) -> Response {
    let LoginForm { email, password } = body.0;

    match validate_credentials(&email, password, db_pool).await {
        Ok(user_id) => {
            session
                .insert("user_id", user_id.to_string())
                .await
                .expect("Failed to insert session");
            session.cycle_id();
            html_response(
                LoginResult {
                    success: true,
                    error_message: String::new(),
                }
                .render()
                .expect("render failed"),
            )
        }
        Err(_) => html_response(
            LoginResult {
                success: false,
                error_message: "Invalid email or password".into(),
            }
            .render()
            .expect("render failed"),
        ),
    }
}

fn html_response(html: String) -> Response {
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}
