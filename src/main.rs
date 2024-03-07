use std::env::current_exe;

use dotenvy::dotenv;
use poem::{
    endpoint::StaticFilesEndpoint, get, http::Method, listener::TcpListener, middleware::Cors,
    EndpointExt, Route, Server,
};

mod services;
mod util;

#[derive(serde::Deserialize, Clone)]
pub struct EnvironmentVariables {
    database_url: String,
    host_address: String,
    spotify_client_id: String,
    spotify_client_secret: String,
    port: u16,
}
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenv().ok();

    let mut path = current_exe()?;
    path.pop();
    path.push("public");

    println!("{:?}", path);

    let env = envy::from_env::<EnvironmentVariables>()?;
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&env.database_url)
        .await?;

    let cors = Cors::new()
        .allow_method(Method::GET)
        .allow_origin_regex("*");

    let app = Route::new()
        .nest(
            "/",
            get(StaticFilesEndpoint::new(path).index_file("index.html")),
        )
        .nest("/auth", services::auth::api())
        .nest("/sse", services::sse::api())
        .data(pool)
        .data(env.clone())
        .with(cors);

    Server::new(TcpListener::bind(format!("0.0.0.0:{}", env.port)))
        .run(app)
        .await?;
    Ok(())
}
