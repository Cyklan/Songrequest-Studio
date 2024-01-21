use poem::{http::Method, listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};

mod services;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let cors = Cors::new()
        .allow_method(Method::GET)
        .allow_origin_regex("*");
    
    let app = Route::new().nest("/sse", services::sse::api()).with(cors);

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
