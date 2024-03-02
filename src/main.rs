use dotenvy::dotenv;
use poem::{http::Method, listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};

mod services;

#[derive(serde::Deserialize, Clone)]
pub struct EnvironmentVariables {
    database_url: String,
    host_address: String,
    spotify_client_id: String,
    spotify_client_secret: String,
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let env = match envy::from_env::<EnvironmentVariables>() {
        Ok(env) => env,
        Err(e) => {
            println!("Failed to load environment variables: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to load environment variables",
            ));
        }
    };

    let pool = match sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&env.database_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            println!("Failed to connect to database: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to connect to database",
            ));
        }
    };

    let cors = Cors::new()
        .allow_method(Method::GET)
        .allow_origin_regex("*");

    let app = Route::new()
        .nest("/auth", services::auth::api())
        .nest("/sse", services::sse::api())
        .data(pool)
        .data(env)
        .with(cors);

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
