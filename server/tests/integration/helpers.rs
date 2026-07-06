use pavex::{config::ConfigLoader, server::Server};
use server::configuration::Profile;
use server_sdk::{ApplicationConfig, ApplicationState, run};
use std::sync::Once;
use tracing::subscriber::set_global_default;
use tracing_subscriber::EnvFilter;

pub struct TestApi {
    pub api_address: String,
    pub api_client: reqwest::Client,
}

impl TestApi {
    pub async fn spawn() -> Self {
        Self::init_telemetry();
        let config = Self::get_config();

        let tcp_listener = config
            .server
            .listener()
            .await
            .expect("Failed to bind the server TCP listener");
        let address = tcp_listener
            .local_addr()
            .expect("The server TCP listener doesn't have a local socket address");
        let server_builder = Server::new().listen(tcp_listener);
        let api_address = format!("http://{}:{}", config.server.ip, address.port());

        let application_state = ApplicationState::new(config)
            .await
            .expect("Failed to build the application state");

        tokio::spawn(async move { run(server_builder, application_state).await });

        TestApi {
            api_address,
            api_client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }

    /// Load the dev configuration and tweak it to ensure that tests are
    /// properly isolated from each other.
    fn get_config() -> ApplicationConfig {
        let mut config: ApplicationConfig = ConfigLoader::new()
            .profile(Profile::Dev)
            .load()
            .expect("Failed to load test configuration");
        // We use port `0` to get the operating system to assign us a random port.
        // This lets us run tests in parallel without running into "port X is already in use"
        // errors.
        config.server.port = 0;
        // Each test gets its own in-memory SQLite DB — fully isolated,
        // no cleanup needed, no interference with the dev DB.
        config.database.database_url = "sqlite::memory:".into();
        config.database.create_if_missing = true;

        // Generate a fresh ed25519 keypair per test run.
        let key_pair = jwt_simple::algorithms::Ed25519KeyPair::generate();
        config.auth.eddsa_public_key_pem = key_pair.public_key().to_pem();
        config.auth.eddsa_private_key_pem = secrecy::Secret::new(key_pair.to_pem());
        config
    }

    fn init_telemetry() {
        // Initialize the telemetry setup at most once.
        static INIT_TELEMETRY: Once = Once::new();
        INIT_TELEMETRY.call_once(|| {
            // Only enable the telemetry if the `TEST_LOG` environment variable is set.
            if std::env::var("TEST_LOG").is_ok() {
                let subscriber = tracing_subscriber::fmt::Subscriber::builder()
                    .with_env_filter(
                        EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")),
                    )
                    .finish();
                // We don't redirect panic messages to the `tracing` subsystem because
                // we want to see them in the test output.
                set_global_default(subscriber).expect("Failed to set a `tracing` global subscriber")
            }
        });
    }
}

/// Convenient methods for calling the API under test.
impl TestApi {
    pub async fn post_signup<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/api/users", &self.api_address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/api/users/login", &self.api_address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_user(&self, token: &str) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/api/user", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn put_user<Body>(&self, token: &str, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .put(&format!("{}/api/user", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn signup_and_get_token(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> String {
        let body = self
            .post_signup(&serde_json::json!({
                "user": {
                    "username": username,
                    "email": email,
                    "password": password,
                }
            }))
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse signup response");

        body["user"]["token"]
            .as_str()
            .expect("Token missing from signup response")
            .to_owned()
    }
}

impl TestApi {
    pub async fn post_label<Body>(&self, token: &str, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/api/labels", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_labels(&self, token: &str) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/api/labels", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn put_label<Body>(&self, token: &str, id: i64, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .put(&format!("{}/api/labels/{id}", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_label(&self, token: &str, id: i64) -> reqwest::Response {
        self.api_client
            .delete(&format!("{}/api/labels/{id}", &self.api_address))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

impl TestApi {
    /// Signs up and returns the session cookie header value, since reqwest's
    /// automatic cookie jar has proven unreliable in this test setup —
    /// every subsequent request must attach this manually.
    pub async fn signup_session(&self, username: &str, email: &str, password: &str) -> String {
        let response = self
            .api_client
            .post(&format!("{}/signup", &self.api_address))
            .form(&[
                ("username", username),
                ("email", email),
                ("password", password),
            ])
            .send()
            .await
            .expect("Failed to execute signup request.");

        assert_eq!(
            response.status().as_u16(),
            200,
            "signup failed: {}",
            response.text().await.unwrap_or_default()
        );

        let cookie_header = response
            .headers()
            .get("set-cookie")
            .expect("no set-cookie header on signup response")
            .to_str()
            .unwrap()
            .to_string();
        cookie_header.split(';').next().unwrap().to_string()
    }

    pub async fn post_todo_page<Body>(&self, cookie: &str, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/todos", &self.api_address))
            .header("Cookie", cookie)
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn put_todo_page<Body>(&self, cookie: &str, id: i64, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .put(&format!("{}/todos/{id}", &self.api_address))
            .header("Cookie", cookie)
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn delete_todo_page(&self, cookie: &str, id: i64) -> reqwest::Response {
        self.api_client
            .delete(&format!("{}/todos/{id}", &self.api_address))
            .header("Cookie", cookie)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_todo_page(&self, cookie: &str, id: i64) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/todos/{id}", &self.api_address))
            .header("Cookie", cookie)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_todos_page(&self, cookie: &str) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/todos", &self.api_address))
            .header("Cookie", cookie)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

#[tokio::test]
async fn debug_cookie_flow() {
    let api = TestApi::spawn().await;
    let signup_resp = api
        .api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[
            ("username", "alice"),
            ("email", "alice@example.com"),
            ("password", "hunter22"),
        ])
        .send()
        .await
        .unwrap();

    println!("signup status: {}", signup_resp.status());
    println!(
        "set-cookie header: {:?}",
        signup_resp.headers().get("set-cookie")
    );

    let todos_resp = api
        .api_client
        .get(&format!("{}/todos", api.api_address))
        .send()
        .await
        .unwrap();

    println!("todos status: {}", todos_resp.status());
    println!("request cookie sent: checking via a raw fetch is hard; redirect location below");
    println!(
        "location header: {:?}",
        todos_resp.headers().get("location")
    );
}

#[tokio::test]
async fn debug_cookie_flow_manual() {
    let api = TestApi::spawn().await;
    let signup_resp = api
        .api_client
        .post(&format!("{}/signup", api.api_address))
        .form(&[
            ("username", "alice"),
            ("email", "alice@example.com"),
            ("password", "hunter22"),
        ])
        .send()
        .await
        .unwrap();

    let cookie_header = signup_resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    // Extract just "session=VALUE" (before the first ';').
    let cookie_value = cookie_header.split(';').next().unwrap();

    let todos_resp = api
        .api_client
        .get(&format!("{}/todos", api.api_address))
        .header("Cookie", cookie_value)
        .send()
        .await
        .unwrap();

    println!("todos status (manual cookie): {}", todos_resp.status());
    println!(
        "location header: {:?}",
        todos_resp.headers().get("location")
    );
}
