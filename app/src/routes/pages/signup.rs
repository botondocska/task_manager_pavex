use crate::routes::api::users::password::compute_password_hash;
use askama::Template;
use htmx_macro::{hx_get, hx_post};
use pavex::{
    Response,
    http::{HeaderValue, header::HeaderName},
    request::body::UrlEncodedBody,
};
use pavex_session::Session;
use secrecy::Secret;
use sqlx::SqlitePool;

#[derive(Template)]
#[template(path = "signup.html")]
struct SignupPage;

#[hx_get(path = "/signup", template = "signup.html")]
pub fn signup_page() -> Response {
    let html = SignupPage.render().expect("template render failed");
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}

#[derive(Template)]
#[template(path = "signup_result.html")]
struct SignupResult {
    success: bool,
    username: String,
    error_message: String,
}

#[derive(serde::Deserialize)]
pub struct SignupForm {
    pub username: String,
    pub email: String,
    pub password: Secret<String>,
}

#[hx_post(path = "/signup", template = "signup_result.html")]
pub async fn signup_submit(
    body: UrlEncodedBody<SignupForm>,
    db_pool: &SqlitePool,
    session: &mut Session<'_>,
) -> Response {
    let SignupForm {
        username,
        email,
        password,
    } = body.0;

    let hash = match compute_password_hash(password) {
        Ok(h) => h,
        Err(e) => return render_error(e.to_string()),
    };

    match insert_user_record(&username, &email, &hash, db_pool).await {
        Ok(user_id) => {
            session
                .insert("user_id", user_id.to_string())
                .await
                .expect("Failed to insert session");
            session.cycle_id();
            redirect_response("/")
        }
        Err(sqlx::Error::Database(ref e)) if e.is_unique_violation() => {
            render_error("That username or email is already taken.".into())
        }
        Err(e) => render_error(format!("Something went wrong: {e}")),
    }
}

fn render_success(username: String) -> Response {
    let html = SignupResult {
        success: true,
        username,
        error_message: String::new(),
    }
    .render()
    .expect("render failed");
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}

fn render_error(msg: String) -> Response {
    let html = SignupResult {
        success: false,
        username: String::new(),
        error_message: msg,
    }
    .render()
    .expect("render failed");
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}

fn redirect_response(location: &'static str) -> Response {
    Response::ok().insert_header(
        HeaderName::from_static("hx-redirect"),
        HeaderValue::from_static(location),
    )
}

async fn insert_user_record(
    username: &str,
    email: &str,
    password_hash: &Secret<String>,
    pool: &SqlitePool,
) -> Result<uuid::Uuid, sqlx::Error> {
    let user_id = uuid::Uuid::new_v4();
    let user_id_str = user_id.to_string();
    let hash = secrecy::ExposeSecret::expose_secret(password_hash);
    sqlx::query!(
        r#"INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)"#,
        user_id_str,
        username,
        email,
        hash,
    )
    .execute(pool)
    .await?;
    Ok(user_id)
}
