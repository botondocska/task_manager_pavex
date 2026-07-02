use crate::helpers::TestApi;
use pavex::http::StatusCode;
use serde_json::json;

// --- Signup ---

#[tokio::test]
async fn signup_works() {
    let api = TestApi::spawn().await;

    let response = api
        .post_signup(&json!({
            "user": {
                "username": "alice",
                "email": "alice@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::CREATED.as_u16());
}

#[tokio::test]
async fn signup_returns_jwt_token() {
    let api = TestApi::spawn().await;

    let response = api
        .post_signup(&json!({
            "user": {
                "username": "alice",
                "email": "alice@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response body");
    assert!(!body["user"]["token"].as_str().unwrap_or("").is_empty());
}

#[tokio::test]
async fn signup_returns_user_details() {
    let api = TestApi::spawn().await;

    let response = api
        .post_signup(&json!({
            "user": {
                "username": "alice",
                "email": "alice@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response body");
    assert_eq!(body["user"]["email"], "alice@example.com");
    assert_eq!(body["user"]["username"], "alice");
}

#[tokio::test]
async fn signup_duplicate_email_returns_conflict() {
    let api = TestApi::spawn().await;

    let body = json!({
        "user": {
            "username": "alice",
            "email": "alice@example.com",
            "password": "hunter22",
        }
    });

    api.post_signup(&body).await;
    let response = api
        .post_signup(&json!({
            "user": {
                "username": "alice2",
                "email": "alice@example.com",  // same email, different username
                "password": "hunter22",
            }
        }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::CONFLICT.as_u16());
}

#[tokio::test]
async fn signup_duplicate_username_returns_conflict() {
    let api = TestApi::spawn().await;

    api.post_signup(&json!({
        "user": {
            "username": "alice",
            "email": "alice@example.com",
            "password": "hunter22",
        }
    }))
    .await;

    let response = api
        .post_signup(&json!({
            "user": {
                "username": "alice",            // same username, different email
                "email": "alice2@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::CONFLICT.as_u16());
}

// --- Login ---

#[tokio::test]
async fn login_works() {
    let api = TestApi::spawn().await;

    api.post_signup(&json!({
        "user": {
            "username": "alice",
            "email": "alice@example.com",
            "password": "hunter22",
        }
    }))
    .await;

    let response = api
        .post_login(&json!({
            "user": {
                "email": "alice@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn login_returns_jwt_token() {
    let api = TestApi::spawn().await;

    api.post_signup(&json!({
        "user": {
            "username": "alice",
            "email": "alice@example.com",
            "password": "hunter22",
        }
    }))
    .await;

    let response = api
        .post_login(&json!({
            "user": {
                "email": "alice@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response body");
    assert!(!body["user"]["token"].as_str().unwrap_or("").is_empty());
}

#[tokio::test]
async fn login_wrong_password_returns_401() {
    let api = TestApi::spawn().await;

    api.post_signup(&json!({
        "user": {
            "username": "alice",
            "email": "alice@example.com",
            "password": "hunter22",
        }
    }))
    .await;

    let response = api
        .post_login(&json!({
            "user": {
                "email": "alice@example.com",
                "password": "wrongpassword",
            }
        }))
        .await;

    assert_eq!(
        response.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );
}

#[tokio::test]
async fn login_unknown_email_returns_401() {
    let api = TestApi::spawn().await;

    let response = api
        .post_login(&json!({
            "user": {
                "email": "nobody@example.com",
                "password": "hunter22",
            }
        }))
        .await;

    assert_eq!(
        response.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );
}

#[tokio::test]
async fn get_user_without_token_returns_401() {
    let api = TestApi::spawn().await;

    let response = api
        .api_client
        .get(&format!("{}/api/user", &api.api_address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        response.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );
}

#[tokio::test]
async fn get_user_with_token_returns_200() {
    let api = TestApi::spawn().await;
    let token = api
        .signup_and_get_token("alice", "alice@example.com", "hunter22")
        .await;
    let response = api.get_user(&token).await;
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn update_user_without_token_returns_401() {
    let api = TestApi::spawn().await;

    let response = api
        .put_user(
            "invalid_token",
            &json!({
                "user": { "bio": "hello" }
            }),
        )
        .await;

    assert_eq!(
        response.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );
}

#[tokio::test]
async fn update_user_bio() {
    let api = TestApi::spawn().await;
    let token = api
        .signup_and_get_token("alice", "alice@example.com", "hunter22")
        .await;

    let response = api
        .put_user(
            &token,
            &json!({
                "user": { "bio": "I love Rust" }
            }),
        )
        .await;

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user"]["bio"], "I love Rust");
}

#[tokio::test]
async fn update_user_partial_fields_only_changes_provided() {
    let api = TestApi::spawn().await;
    let token = api
        .signup_and_get_token("alice", "alice@example.com", "hunter22")
        .await;

    api.put_user(
        &token,
        &json!({
            "user": { "bio": "I love Rust" }
        }),
    )
    .await;

    // update only image, bio should stay
    let response = api
        .put_user(
            &token,
            &json!({
                "user": { "image": "https://example.com/avatar.png" }
            }),
        )
        .await;

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["user"]["bio"], "I love Rust");
    assert_eq!(body["user"]["image"], "https://example.com/avatar.png");
}

#[tokio::test]
async fn update_user_password_allows_login_with_new_password() {
    let api = TestApi::spawn().await;
    let token = api
        .signup_and_get_token("alice", "alice@example.com", "hunter22")
        .await;

    api.put_user(
        &token,
        &json!({
            "user": { "password": "newpassword99" }
        }),
    )
    .await;

    // old password should fail
    let old_login = api
        .post_login(&json!({
            "user": { "email": "alice@example.com", "password": "hunter22" }
        }))
        .await;
    assert_eq!(
        old_login.status().as_u16(),
        StatusCode::UNAUTHORIZED.as_u16()
    );

    // new password should work
    let new_login = api
        .post_login(&json!({
            "user": { "email": "alice@example.com", "password": "newpassword99" }
        }))
        .await;
    assert_eq!(new_login.status().as_u16(), StatusCode::OK.as_u16());
}
