use crate::helpers::TestApi;
use pavex::http::StatusCode;

#[tokio::test]
async fn todos_page_returns_200() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api.get_todos_page(&cookie).await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn create_one_off_todo_works() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .post_todo_page(&cookie, &[("title", "Buy milk"), ("description", "2%")])
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body = response.text().await.unwrap();
    assert!(body.contains("Buy milk"));
}

#[tokio::test]
async fn create_todo_without_title_returns_error() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .post_todo_page(&cookie, &[("description", "no title")])
        .await;

    assert!(
        response.status().as_u16() >= 400,
        "expected an error status, got {}",
        response.status()
    );
}

#[tokio::test]
async fn create_recurring_todo_with_valid_rrule_works() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .post_todo_page(
            &cookie,
            &[
                ("title", "Standup"),
                ("repeat", "on"),
                ("dt_start", "2026-07-10T09:00"),
                ("freq", "weekly"),
                ("interval", "1"),
                ("by_weekday", "mon"),
                ("end_type", "never"),
            ],
        )
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body = response.text().await.unwrap();
    assert!(body.contains("Repeats"));
}

#[tokio::test]
async fn create_recurring_todo_with_incomplete_rrule_returns_400() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .post_todo_page(&cookie, &[("title", "Broken recurrence"), ("repeat", "on")])
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::BAD_REQUEST.as_u16());
}

#[tokio::test]
async fn update_todo_title_works() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let create_resp = api.post_todo_page(&cookie, &[("title", "Original")]).await;
    let body = create_resp.text().await.unwrap();
    let id = extract_todo_id(&body);

    let response = api
        .put_todo_page(&cookie, id, &[("title", "Updated title")])
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body = response.text().await.unwrap();
    assert!(body.contains("Updated title"));
}

#[tokio::test]
async fn toggle_completed_persists() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let create_resp = api
        .post_todo_page(&cookie, &[("title", "Mark me done")])
        .await;
    let body = create_resp.text().await.unwrap();
    let id = extract_todo_id(&body);

    let response = api
        .put_todo_page(
            &cookie,
            id,
            &[("title", "Mark me done"), ("completed", "true")],
        )
        .await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

    let response = api
        .put_todo_page(&cookie, id, &[("title", "Mark me done")])
        .await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn delete_todo_works() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let create_resp = api.post_todo_page(&cookie, &[("title", "Delete me")]).await;
    let body = create_resp.text().await.unwrap();
    let id = extract_todo_id(&body);

    let response = api.delete_todo_page(&cookie, id).await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

    let follow_up = api.get_todo_page(&cookie, id).await;
    assert_eq!(follow_up.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn delete_nonexistent_todo_returns_404() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api.delete_todo_page(&cookie, 999_999).await;
    assert_eq!(response.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn update_other_users_todo_returns_404() {
    let api = TestApi::spawn().await;
    let alice_cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let create_resp = api
        .post_todo_page(&alice_cookie, &[("title", "Alice's todo")])
        .await;
    let body = create_resp.text().await.unwrap();
    let id = extract_todo_id(&body);

    let bob_cookie = api
        .signup_session("bob", "bob@example.com", "hunter22")
        .await;

    let response = api
        .put_todo_page(&bob_cookie, id, &[("title", "Hijacked")])
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn delete_other_users_todo_returns_404() {
    let api = TestApi::spawn().await;
    let alice_cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let create_resp = api
        .post_todo_page(&alice_cookie, &[("title", "Alice's todo")])
        .await;
    let body = create_resp.text().await.unwrap();
    let id = extract_todo_id(&body);

    let bob_cookie = api
        .signup_session("bob", "bob@example.com", "hunter22")
        .await;

    let response = api.delete_todo_page(&bob_cookie, id).await;
    assert_eq!(response.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}

fn extract_todo_id(html: &str) -> i64 {
    let marker = "id=\"todo-";
    let start = html.find(marker).expect("todo id marker not found") + marker.len();
    let end = html[start..].find('"').expect("closing quote not found") + start;
    html[start..end]
        .parse()
        .expect("todo id was not a valid i64")
}

#[tokio::test]
async fn flashcards_page_returns_200() {
    let api = TestApi::spawn().await;
    let cookie = api
        .signup_session("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .api_client
        .get(&format!("{}/todos/flashcards", api.api_address))
        .header("Cookie", cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}
