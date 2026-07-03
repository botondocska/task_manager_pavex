use crate::helpers::TestApi;
use pavex::http::StatusCode;
use serde_json::json;

async fn signed_up_token(api: &TestApi) -> String {
    api.signup_and_get_token("alice", "alice@example.com", "hunter22")
        .await
}

#[tokio::test]
async fn create_label_works() {
    let api = TestApi::spawn().await;
    let token = signed_up_token(&api).await;

    let response = api
        .post_label(&token, &json!({ "name": "Work", "color": "#ff0000" }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::CREATED.as_u16());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["label"]["name"], "Work");
    assert_eq!(body["label"]["color"], "#ff0000");
}

#[tokio::test]
async fn create_label_without_token_returns_401() {
    let api = TestApi::spawn().await;

    let response = api
        .api_client
        .post(&format!("{}/api/labels", &api.api_address))
        .json(&json!({ "name": "Work", "color": "#ff0000" }))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        response.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );
}

#[tokio::test]
async fn list_labels_returns_only_own_labels() {
    let api = TestApi::spawn().await;
    let alice_token = signed_up_token(&api).await;
    let bob_token = api
        .signup_and_get_token("bob", "bob@example.com", "hunter22")
        .await;

    api.post_label(&alice_token, &json!({ "name": "Alice's", "color": "#ff0000" }))
        .await;
    api.post_label(&bob_token, &json!({ "name": "Bob's", "color": "#00ff00" }))
        .await;

    let response = api.get_labels(&alice_token).await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

    let body: serde_json::Value = response.json().await.unwrap();
    let labels = body["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["name"], "Alice's");
}

#[tokio::test]
async fn update_label_works() {
    let api = TestApi::spawn().await;
    let token = signed_up_token(&api).await;

    let created = api
        .post_label(&token, &json!({ "name": "Work", "color": "#ff0000" }))
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = created["label"]["id"].as_i64().unwrap();

    let response = api
        .put_label(&token, id, &json!({ "name": "Personal" }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["label"]["name"], "Personal");
    assert_eq!(body["label"]["color"], "#ff0000"); // unchanged
}

#[tokio::test]
async fn update_other_users_label_returns_404() {
    let api = TestApi::spawn().await;
    let alice_token = signed_up_token(&api).await;
    let bob_token = api
        .signup_and_get_token("bob", "bob@example.com", "hunter22")
        .await;

    let created = api
        .post_label(&alice_token, &json!({ "name": "Alice's", "color": "#ff0000" }))
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = created["label"]["id"].as_i64().unwrap();

    let response = api
        .put_label(&bob_token, id, &json!({ "name": "Hijacked" }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn delete_label_works() {
    let api = TestApi::spawn().await;
    let token = signed_up_token(&api).await;

    let created = api
        .post_label(&token, &json!({ "name": "Work", "color": "#ff0000" }))
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = created["label"]["id"].as_i64().unwrap();

    let response = api.delete_label(&token, id).await;
    assert_eq!(response.status().as_u16(), StatusCode::NO_CONTENT.as_u16());

    let list = api.get_labels(&token).await;
    let body: serde_json::Value = list.json().await.unwrap();
    assert_eq!(body["labels"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn delete_other_users_label_returns_404() {
    let api = TestApi::spawn().await;
    let alice_token = signed_up_token(&api).await;
    let bob_token = api
        .signup_and_get_token("bob", "bob@example.com", "hunter22")
        .await;

    let created = api
        .post_label(&alice_token, &json!({ "name": "Alice's", "color": "#ff0000" }))
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let id = created["label"]["id"].as_i64().unwrap();

    let response = api.delete_label(&bob_token, id).await;
    assert_eq!(response.status().as_u16(), StatusCode::NOT_FOUND.as_u16());
}