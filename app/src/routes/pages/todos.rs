use crate::{
    routes::api::{labels::repo as labels_repo, todos::repo as todos_repo},
    rrule_input::{RRuleField, RRuleInput, build_rrule_set},
    schemas::{CreateTodoBody, Label, Todo, UpdateTodoBody},
    session_auth::SessionUserId,
};
use askama::Template;
use htmx_macro::{hx_delete, hx_get, hx_post, hx_put};
use pavex::{
    Response,
    http::HeaderValue,
    request::{body::UrlEncodedBody, path::PathParams},
};
use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// Flat form shape — separate from CreateTodoBody/UpdateTodoBody (API, nested).
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct TodoForm {
    pub title: String,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub label_id: Option<i64>,
    pub repeat: Option<String>,
    pub dt_start: Option<String>,
    pub freq: Option<String>,
    pub interval: Option<u16>,
    pub by_weekday: Option<Vec<String>>,
    pub end_type: Option<String>,
    pub count: Option<u32>,
    pub until: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TodoFormError {
    #[error("Recurrence fields incomplete: {0}")]
    IncompleteRRule(String),
}

fn build_rrule_from_form(form: &TodoForm) -> Result<Option<RRuleField>, TodoFormError> {
    if form.repeat.is_none() {
        return Ok(None);
    }

    let raw = RRuleInput {
        dt_start: form
            .dt_start
            .clone()
            .ok_or_else(|| TodoFormError::IncompleteRRule("dt_start missing".into()))?,
        freq: form
            .freq
            .clone()
            .ok_or_else(|| TodoFormError::IncompleteRRule("freq missing".into()))?,
        interval: form.interval,
        by_weekday: form.by_weekday.clone(),
        end_type: form
            .end_type
            .clone()
            .ok_or_else(|| TodoFormError::IncompleteRRule("end_type missing".into()))?,
        count: form.count,
        until: form.until.clone(),
    };

    let set = build_rrule_set(raw).map_err(|e| TodoFormError::IncompleteRRule(e.to_string()))?;
    Ok(Some(RRuleField(set)))
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "todos.html")]
struct TodosPage {
    todos: Vec<Todo>,
    labels: Vec<Label>,
    active_page: &'static str,
}

#[derive(Template)]
#[template(path = "todo_row.html")]
struct TodoRow {
    todo: Todo,
    labels: Vec<Label>,
}

#[derive(Template)]
#[template(path = "todo_edit_row.html")]
struct TodoEditRow {
    todo: Todo,
    labels: Vec<Label>,
    rrule_raw: Option<RRuleInput>,
}
// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum TodosPageError {
    #[error("Todo not found.")]
    NotFound,
    #[error("Invalid recurrence rule: {0}")]
    BadRRule(#[from] TodoFormError),
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[pavex::methods]
impl TodosPageError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        let (status, message) = match self {
            TodosPageError::NotFound => (Response::not_found(), "Todo not found.".to_string()),
            TodosPageError::BadRRule(e) => (
                Response::bad_request(),
                format!("Recurrence rule problem: {e}"),
            ),
            TodosPageError::UnexpectedError(_) => (
                Response::internal_server_error(),
                "Something went wrong. Please try again.".to_string(),
            ),
        };

        status.set_typed_body(message).insert_header(
            pavex::http::header::CONTENT_TYPE,
            pavex::http::HeaderValue::from_static("text/plain; charset=utf-8"),
        )
    }
}

fn html_response(html: String) -> Response {
    Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    )
}

// ---------------------------------------------------------------------------
// GET / and /todos — full page
// ---------------------------------------------------------------------------

#[hx_get(path = "/", template = "todos.html")]
pub async fn home_page(
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    render_todos_page(user, pool, "todos").await
}

//#[hx_get(path = "/todos/{id}", template = "todo_row.html")]
#[hx_get(path = "/todos/{id}")]
pub async fn get_todo_page(
    params: PathParams<TodoIdParam>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    let TodoIdParam { id } = params.0;
    let user_id = user.0.to_string();

    let todos = todos_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;
    let todo = todos
        .into_iter()
        .find(|t| t.id == id)
        .ok_or(TodosPageError::NotFound)?;
    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let html = TodoRow { todo, labels }.render().expect("render failed");
    Ok(html_response(html))
}

#[hx_get(path = "/todos", template = "todos.html")]
pub async fn todos_page(
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    render_todos_page(user, pool, "todos").await
}

async fn render_todos_page(
    user: &SessionUserId,
    pool: &SqlitePool,
    active_page: &'static str,
) -> Result<Response, TodosPageError> {
    let user_id = user.0.to_string();

    let todos = todos_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;
    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let html = TodosPage {
        todos,
        labels,
        active_page,
    }
        .render()
        .expect("template render failed");
    Ok(html_response(html))
}

// ---------------------------------------------------------------------------
// POST /todos — create, returns new row fragment
// ---------------------------------------------------------------------------

//#[hx_post(path = "/todos", template = "todo_row.html")]
#[hx_post(path = "/todos")]
pub async fn create_todo_page(
    body: UrlEncodedBody<TodoForm>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    let form = body.0;
    let user_id = user.0.to_string();

    let rrule = build_rrule_from_form(&form)?;

    let create_body = CreateTodoBody {
        title: form.title,
        description: form.description,
        duration: form.duration,
        rrule,
        label_id: form.label_id,
    };

    let todo = todos_repo::create(&user_id, &create_body, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let html = TodoRow { todo, labels }.render().expect("render failed");
    Ok(html_response(html))
}

// ---------------------------------------------------------------------------
// GET /todos/{id}/edit — swap row for inline edit form
// ---------------------------------------------------------------------------

#[PathParams]
pub struct TodoIdParam {
    pub id: i64,
}

//#[hx_get(path = "/todos/{id}/edit", template = "todo_edit_row.html")]
#[hx_get(path = "/todos/{id}/edit")]
pub async fn edit_todo_page(
    params: PathParams<TodoIdParam>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    let TodoIdParam { id } = params.0;
    let user_id = user.0.to_string();

    let todos = todos_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;
    let todo = todos
        .into_iter()
        .find(|t| t.id == id)
        .ok_or(TodosPageError::NotFound)?;

    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let rrule_raw = todo
        .rrule
        .as_ref()
        .map(|r| crate::rrule_input::parse_rrule_string(&r.0.to_string()))
        .transpose()
        .map_err(TodosPageError::UnexpectedError)?;

    let html = TodoEditRow {
        todo,
        labels,
        rrule_raw,
    }
    .render()
    .expect("render failed");
    Ok(html_response(html))
}

// ---------------------------------------------------------------------------
// PUT /todos/{id} — save edit, returns updated row fragment (view mode)
// ---------------------------------------------------------------------------

//#[hx_put(path = "/todos/{id}", template = "todo_row.html")]
#[hx_put(path = "/todos/{id}")]
pub async fn update_todo_page(
    params: PathParams<TodoIdParam>,
    body: UrlEncodedBody<TodoForm>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    let TodoIdParam { id } = params.0;
    let form = body.0;
    let user_id = user.0.to_string();

    let rrule = build_rrule_from_form(&form)?;

    let update_body = UpdateTodoBody {
        title: Some(form.title),
        description: form.description,
        duration: form.duration,
        rrule,
        label_id: form.label_id,
    };

    let todo = todos_repo::update(&user_id, id, &update_body, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?
        .ok_or(TodosPageError::NotFound)?;

    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    let html = TodoRow { todo, labels }.render().expect("render failed");
    Ok(html_response(html))
}

// ---------------------------------------------------------------------------
// DELETE /todos/{id}
// ---------------------------------------------------------------------------

#[hx_delete(path = "/todos/{id}")]
pub async fn delete_todo_page(
    params: PathParams<TodoIdParam>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, TodosPageError> {
    let TodoIdParam { id } = params.0;
    let user_id = user.0.to_string();

    let deleted = todos_repo::delete(&user_id, id, pool)
        .await
        .map_err(|e| TodosPageError::UnexpectedError(e.into()))?;

    if !deleted {
        return Err(TodosPageError::NotFound);
    }

    Ok(Response::ok().set_typed_body(""))
}
