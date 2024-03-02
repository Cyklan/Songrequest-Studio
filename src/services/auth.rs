use poem::{
    error::ResponseError,
    get, handler,
    http::StatusCode,
    web::{Data, Query, Redirect},
    IntoResponse, Route,
};
use serde::{Deserialize, Serialize};
use sqlx::pool;

use crate::EnvironmentVariables;

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
struct AccessTokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
}

#[derive(Deserialize)]
struct UserProfileResponse {
    uri: String,
}

struct MyError;

impl ResponseError for MyError {
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl IntoResponse for MyError {
    fn into_response(self) -> poem::Response {
        poem::Response::builder()
            .status(self.status())
            .body("Internal Server Error")
    }
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
) -> impl IntoResponse {
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
        .await;

    let response = match response {
        Ok(response) => response,
        Err(_) => return MyError.into_response(),
    };

    let token = match response.json::<AccessTokenResponse>().await {
        Ok(token) => token,
        Err(_) => return MyError.into_response(),
    };

    let user_request = client
        .get("https://api.spotify.com/v1/me")
        .bearer_auth(token.access_token.clone())
        .send()
        .await;

    let user_response = match user_request {
        Ok(response) => response.json::<UserProfileResponse>().await,
        Err(_) => return MyError.into_response(),
    };

    let user = match user_response {
        Ok(user) => user,
        Err(_) => return MyError.into_response(),
    };

    let db_result = sqlx::query!(
        "INSERT INTO auth (access_token, expiry_date, refresh_token) VALUES (?, ?, ?)",
        token.access_token,
        chrono::Utc::now().timestamp() + token.expires_in,
        token.refresh_token
    )
    .execute(&**pool)
    .await;

    match db_result {
        Ok(res) => {
            let _ = sqlx::query!("INSERT INTO user (username, auth_id) VALUES (?, ?) ON DUPLICATE KEY UPDATE auth_id = ?",
                user.uri,
                res.last_insert_id(),
                res.last_insert_id()
            ).execute(&**pool).await;

            return Redirect::temporary(format!("/?uri={}", user.uri)).into_response();
        }
        Err(_) => return MyError.into_response(),
    }
}

pub fn api() -> Route {
    return Route::new()
        .at("/", get(index))
        .at("/callback", get(callback));
}
