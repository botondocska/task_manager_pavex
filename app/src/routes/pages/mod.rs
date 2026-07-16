mod calendar;
mod labels;
mod login;
mod logout;
mod nav;
mod signup;
mod theme;
mod todo_history;
mod todos;

pub use calendar::*;
pub use labels::*;
pub use login::*;
pub use logout::*;
pub use nav::*;
pub use signup::*;
pub use theme::*;
pub use todo_history::*;
pub use todos::*;

use pavex::Response;

pub fn html_response(html: String) -> Response {
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        pavex::http::HeaderValue::from_static("text/html; charset=utf-8"),
    )
}
