use std::time::Duration;

use color_eyre::eyre::Result;
use poem::{
    get, handler,
    web::{
        sse::{Event, SSE},
        Data, Query,
    },
    Route,
};
use serde::{Deserialize, Serialize};
use sqlx::{pool, prelude::FromRow};
use stream::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::{self as stream, wrappers::ReceiverStream};

use crate::EnvironmentVariables;

#[derive(Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Serialize)]
struct RefreshRequest {
    refresh_token: String,
    grant_type: String,
}

#[derive(Deserialize, FromRow)]
struct Auth {
    access_token: String,
    expiry_date: i64,
    refresh_token: String,
}

#[derive(Deserialize)]
struct SSEQuery {
    uri: String,
}

enum Response {
    Success(SongResponse),
    Error(ErrorResponse),
}

#[derive(Serialize)]
struct SongResponse {
    title: String,
    artist: String,
    album_cover: String,
    progress_ms: f32,
    total_ms: f32,
    is_playing: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct Song {
    progress_ms: f32,
    is_playing: bool,
    item: SongItem,
}

#[derive(Deserialize)]
struct SongItem {
    name: String,
    duration_ms: f32,
    album: Album,
    artists: Vec<Artist>,
}

#[derive(Deserialize)]
struct Album {
    images: Vec<Image>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}

#[handler]
fn index(
    pool: Data<&pool::Pool<sqlx::MySql>>,
    env: Data<&EnvironmentVariables>,
    Query(SSEQuery { uri }): Query<SSEQuery>,
) -> SSE {
    let pool_clone = pool.clone();
    let env_clone = env.clone();
    let (tx, rx) = mpsc::channel::<Response>(32);
    tokio::spawn(async move { check_spotify(tx, uri, &pool_clone, &env_clone).await });
    let stream = ReceiverStream::new(rx).map(|msg| {
        let msg = match msg {
            Response::Success(song) => serde_json::to_string(&song).unwrap(),
            Response::Error(err) => serde_json::to_string(&err).unwrap(),
        };
        Event::message(msg)
    });

    return SSE::new(stream);
}

async fn check_spotify(
    tx: mpsc::Sender<Response>,
    uri: String,
    pool: &pool::Pool<sqlx::MySql>,
    env: &EnvironmentVariables,
) -> Result<()> {
    let mut user = sqlx::query_as!(
        Auth,
        "SELECT auth.access_token, auth.expiry_date, auth.refresh_token
    FROM user
    JOIN auth ON user.auth_id = auth.id
    WHERE user.username = ?",
        uri
    )
    .fetch_one(pool)
    .await?;

    let client = reqwest::Client::new();

    loop {
        if (user.expiry_date - chrono::Utc::now().timestamp()) < 60 {
            let request_body = RefreshRequest {
                refresh_token: user.refresh_token.clone(),
                grant_type: "refresh_token".to_string(),
            };

            let response = client
                .post("https://accounts.spotify.com/api/token")
                .basic_auth(
                    env.spotify_client_id.clone(),
                    Some(env.spotify_client_secret.clone()),
                )
                .form(&request_body)
                .send()
                .await?
                .json::<AccessTokenResponse>()
                .await?;

            let _ = sqlx::query_as!(
                Auth,
                "INSERT INTO auth (access_token, expiry_date, refresh_token) VALUES (?, ?, ?)",
                response.access_token,
                chrono::Utc::now().timestamp() + response.expires_in,
                user.refresh_token
            )
            .execute(pool)
            .await?;

            user = Auth {
                access_token: response.access_token,
                expiry_date: chrono::Utc::now().timestamp() + response.expires_in,
                refresh_token: user.refresh_token,
            }
        }

        let song_res = client
            .get("https://api.spotify.com/v1/me/player")
            .bearer_auth(user.access_token.clone())
            .send()
            .await;

        let res_content = match song_res {
            Ok(res) => res,
            Err(_) => {
                if let Err(_) = tx
                    .send(Response::Error(ErrorResponse {
                        error: "Spotify API error".to_string(),
                    }))
                    .await
                {
                    break;
                };
                tokio::time::sleep(Duration::from_millis(10_000)).await;
                continue;
            }
        };

        let song_item = res_content.json::<Song>().await;
        let song = match song_item {
            Ok(song) => song,
            Err(error) => {
                if let Err(_) = tx
                    .send(Response::Error(ErrorResponse {
                        error: error.to_string(), // api key expired, should refresh (shouldn't really occur because )
                    }))
                    .await
                {
                    break;
                };
                tokio::time::sleep(Duration::from_millis(10_000)).await;
                continue;
            }
        };

        let res = SongResponse {
            album_cover: song.item.album.images[0].url.clone(),
            is_playing: song.is_playing,
            title: song.item.name,
            progress_ms: song.progress_ms,
            total_ms: song.item.duration_ms,
            // for artist iterate over all artists and join them with a comma
            artist: song
                .item
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<String>>()
                .join(", "),
        };

        if let Err(_) = tx.send(Response::Success(res)).await {
            break;
        };

        let diff = song.item.duration_ms - song.progress_ms;
        let check_time = if diff < 20_000.0 {
            (diff + 200.0) as u64
        } else {
            20_000
        };
        tokio::time::sleep(Duration::from_millis(check_time)).await;
    }
    Ok(())
}

pub fn api() -> Route {
    return Route::new().at("/", get(index));
}
