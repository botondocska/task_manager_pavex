// app/src/session_auth.rs
use pavex::Response;
use pavex_session::Session;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct SessionUserId(pub Uuid);

#[derive(Debug, thiserror::Error)]
#[error("Not logged in")]
pub struct NotLoggedIn;

#[pavex::methods]
impl SessionUserId {
    #[request_scoped]
    pub async fn extract(session: &Session<'_>) -> Result<Self, NotLoggedIn> {
        let raw = session
            .get::<String>("user_id")
            .await
            .map_err(|_| NotLoggedIn)?
            .ok_or(NotLoggedIn)?;
        let id = Uuid::parse_str(&raw).map_err(|_| NotLoggedIn)?;
        Ok(SessionUserId(id))
    }

    #[error_handler]
    pub fn not_logged_in(_e: &NotLoggedIn) -> Response {
        Response::see_other().insert_header(
            pavex::http::header::LOCATION,
            pavex::http::HeaderValue::from_static("/login"),
        )
    }
}
