use crate::helpers::TestApi;
use pavex::http::StatusCode;

#[tokio::test]
async fn signup_page_returns_200() {
    let api = TestApi::spawn().await;
    let response = api.api_client
        .get(&format!("{}/signup", api.api_address))
        .send().await.unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn signup_submit_creates_session_cookie() {
    let api = TestApi::spawn().await;
    let response = api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[("username", "alice"), ("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert!(response.headers().get("set-cookie").is_some(), "session cookie should be set");
}

#[tokio::test]
async fn signup_duplicate_email_returns_error_html() {
    let api = TestApi::spawn().await;
    let form = [("username", "alice"), ("email", "alice@example.com"), ("password", "hunter22")];
    api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&form).send().await.unwrap();

    let response = api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[("username", "alice2"), ("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body = response.text().await.unwrap();
    assert!(body.contains("already taken"), "should show conflict error");
}

#[tokio::test]
async fn login_page_returns_200() {
    let api = TestApi::spawn().await;
    let response = api.api_client
        .get(&format!("{}/login", api.api_address))
        .send().await.unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
}

#[tokio::test]
async fn login_submit_sets_session_cookie() {
    let api = TestApi::spawn().await;
    api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[("username", "alice"), ("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    let response = api.api_client
        .post(&format!("{}/login", api.api_address))
        .form(&[("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert!(response.headers().get("set-cookie").is_some(), "session cookie should be set");
}

#[tokio::test]
async fn login_wrong_password_returns_error_html() {
    let api = TestApi::spawn().await;
    api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[("username", "alice"), ("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    let response = api.api_client
        .post(&format!("{}/login", api.api_address))
        .form(&[("email", "alice@example.com"), ("password", "wrongpassword")])
        .send().await.unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body = response.text().await.unwrap();
    assert!(body.contains("Invalid email or password"));
}

#[tokio::test]
async fn logout_clears_session() {
    let api = TestApi::spawn().await;
    // signup to get session
    api.api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[("username", "alice"), ("email", "alice@example.com"), ("password", "hunter22")])
        .send().await.unwrap();

    let response = api.api_client
        .post(&format!("{}/logout", api.api_address))
        .send().await.unwrap();

    // should redirect to login
    assert_eq!(response.status().as_u16(), StatusCode::SEE_OTHER.as_u16());
}