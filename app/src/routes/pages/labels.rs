use crate::session_theme::Theme;
use crate::{
    routes::{api::labels::repo, pages::nav::NAV_ITEMS},
    schemas::{CreateLabelBody, Label, UpdateLabelBody},
    session_auth::SessionUserId,
};
use askama::Template;
use pavex::{
    Response, delete, get,
    http::HeaderValue,
    post, put,
    request::{body::UrlEncodedBody, path::PathParams},
};
use sqlx::SqlitePool;

#[derive(Template)]
#[template(path = "labels.html")]
struct LabelsPage {
    labels: Vec<Label>,
    active_page: &'static str,
    nav_items: &'static [crate::routes::pages::nav::NavItem],
    theme: Theme,
}

#[get(path = "/labels")]
pub async fn labels_page(
    user: &SessionUserId,
    pool: &SqlitePool,
    theme: &Theme,
) -> Result<Response, LabelsPageError> {
    let labels = repo::list_for_user(&user.0.to_string(), pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?;

    let html = LabelsPage {
        labels,
        active_page: "labels",
        nav_items: NAV_ITEMS,
        theme: *theme,
    }
    .render()
    .expect("template render failed");
    Ok(html_response(html))
}

#[derive(Template)]
#[template(path = "label_row.html")]
struct LabelRow {
    label: Label,
}

#[post(path = "/labels")]
pub async fn create_label_page(
    body: UrlEncodedBody<CreateLabelBody>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, LabelsPageError> {
    let user_id = user.0.to_string();

    let label = repo::create(&user_id, &body.0, pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?;

    let html = LabelRow { label }.render().expect("render failed");
    Ok(html_response(html))
}

#[PathParams]
pub struct LabelIdParam {
    pub id: i64,
}

#[put(path = "/labels/{id}")]
pub async fn update_label_page(
    params: PathParams<LabelIdParam>,
    body: UrlEncodedBody<UpdateLabelBody>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, LabelsPageError> {
    let LabelIdParam { id } = params.0;
    let user_id = user.0.to_string();

    let label = repo::update(&user_id, id, &body.0, pool)
        .await
        .map_err(|e| LabelsPageError::UnexpectedError(e.into()))?
        .ok_or_else(|| LabelsPageError::UnexpectedError(anyhow::anyhow!("Label not found")))?;

    let html = LabelRow { label }.render().expect("render failed");
    Ok(html_response(html))
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
