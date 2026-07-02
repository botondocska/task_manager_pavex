use pavex::{Response, post, http::HeaderValue};
use pavex_session::Session;

#[post(path = "/logout")]
pub async fn logout(session: &mut Session<'_>) -> Response {
    session.invalidate();
    // redirect to login
    Response::see_other()
        .append_header(pavex::http::header::LOCATION, HeaderValue::from_static("/login"))
}