use poem::{
    get, handler,
    web::{Data, Query, Redirect},
    Route,
};
use serde::{Deserialize, Serialize};
use sqlx::pool;

use crate::{util::error::HttpError, EnvironmentVariables};

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
}

#[derive(Serialize)]
struct AccessTokenRequest {
    grant_type: String,
    code: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: String,
}

#[derive(Deserialize)]
struct UserProfileResponse {
    uri: String,
}

#[handler]
fn index(env: Data<&EnvironmentVariables>) -> Redirect {
    return Redirect::temporary(format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type={}&redirect_uri={}&scope={}",
        env.0.spotify_client_id.clone(),
        "code".to_string(),
        env.0.host_address.clone() + "/auth/callback",
        "user-read-email,user-read-private,user-read-playback-state".to_string()
    ));
}

#[handler]
async fn callback(
    Query(CallbackQuery { code }): Query<CallbackQuery>,
    env: Data<&EnvironmentVariables>,
    pool: Data<&pool::Pool<sqlx::MySql>>,
) -> poem::Result<Redirect, HttpError> {
    let request_body = AccessTokenRequest {
        grant_type: "authorization_code".to_string(),
        code: code.clone(),
        redirect_uri: env.0.host_address.clone() + "/auth/callback",
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(
            env.0.spotify_client_id.clone(),
            Some(env.0.spotify_client_secret.clone()),
        )
        .form(&request_body)
        .send()
        .await?;

    let token = response.json::<AccessTokenResponse>().await?;

    let user_response = client
        .get("https://api.spotify.com/v1/me")
        .bearer_auth(token.access_token.clone())
        .send()
        .await?
        .json::<UserProfileResponse>()
        .await?;

    let db_result = sqlx::query!(
        "INSERT INTO auth (access_token, expiry_date, refresh_token) VALUES (?, ?, ?)",
        token.access_token,
        chrono::Utc::now().timestamp() + token.expires_in,
        token.refresh_token
    )
    .execute(&**pool)
    .await?;

    let _ = sqlx::query!(
        "INSERT INTO user (username, auth_id) VALUES (?, ?) ON DUPLICATE KEY UPDATE auth_id = ?",
        user_response.uri,
        db_result.last_insert_id(),
        db_result.last_insert_id()
    )
    .execute(&**pool)
    .await?;

    Ok(Redirect::temporary(format!("/?uri={}", user_response.uri)))
}

pub fn api() -> Route {
    return Route::new()
        .at("/", get(index))
        .at("/callback", get(callback));
}
