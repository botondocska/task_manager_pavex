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