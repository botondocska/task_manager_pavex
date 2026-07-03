use crate::{routes::api::labels::repo, schemas::Label, session_auth::SessionUserId};
use askama::Template;
use htmx_macro::{hx_delete, hx_get, hx_post};
use pavex::{
    Response, delete, get,
    http::HeaderValue,
    post,
    request::{body::UrlEncodedBody, path::PathParams},
};
use sqlx::SqlitePool;

#[derive(Template)]
#[template(path = "labels.html")]
struct LabelsPage {
    labels: Vec<Label>,
}

#[get(path = "/labels")]
pub async fn labels_page(
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, LabelsPageError> {
    let labels = repo::list_for_user(&user.0.to_string(), pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?;

    let html = LabelsPage { labels }
        .render()
        .expect("template render failed");
    Ok(html_response(html))
}

#[derive(Template)]
#[template(path = "label_row.html")]
struct LabelRow {
    label: Label,
}

#[derive(serde::Deserialize)]
pub struct CreateLabelForm {
    pub name: String,
    pub color: String,
}

#[post(path = "/labels")]
pub async fn create_label_page(
    body: UrlEncodedBody<CreateLabelForm>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, LabelsPageError> {
    let CreateLabelForm { name, color } = body.0;
    let user_id = user.0.to_string();

    let label = repo::create(&user_id, &name, &color, pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?;

    let html = LabelRow { label }.render().expect("render failed");
    Ok(html_response(html))
}

#[PathParams]
pub struct LabelIdParam {
    pub id: i64,
}

#[delete(path = "/labels/{id}")]
pub async fn delete_label_page(
    params: PathParams<LabelIdParam>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, LabelsPageError> {
    let LabelIdParam { id } = params.0;
    let user_id = user.0.to_string();

    repo::delete(&user_id, id, pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?;

    // htmx swaps this element out with an empty response → element removed.
    Ok(Response::ok().set_typed_body(""))
}

fn html_response(html: String) -> Response {
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}

#[derive(Debug, thiserror::Error)]
pub enum LabelsPageError {
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[pavex::methods]
impl LabelsPageError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        Response::internal_server_error().set_typed_body(format!("{self}"))
    }
}
