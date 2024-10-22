//! Run with
//!
//! ```not_rust
//! cargo run -p example-static-file-server
//! ```

use axum::{
    routing::get, Json, Router
};
use log::info;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

pub async fn start_webserver(port: u16) {

        serve(using_serve_dir(), port).await;

    // tokio::join!(
    //     serve(using_serve_dir(), port),
    //     // serve(using_serve_dir_with_assets_fallback(), 3002),
    //     // serve(using_serve_dir_only_from_root_via_fallback(), 3003),
    //     // serve(using_serve_dir_with_handler_as_service(), 3004),
    //     // serve(two_serve_dirs(), 3005),
    //     // serve(calling_serve_dir_from_a_handler(), 3006),
    //     // serve(using_serve_file_from_a_route(), 3307),
    // );
}

fn using_serve_dir() -> Router {

    // serve the file in the "dist" directory under `/`
    Router::new()
    .route("/api/test", get(test_api))
    .nest_service("/", ServeDir::new("frontend/dist"))
}

#[derive(Serialize, Deserialize)]
struct TestResponse {
    message: String,
}


//#[axum::debug_handler]
async fn test_api() -> Json<TestResponse> {

    Json(TestResponse {
        message: "hello test".to_string(),
    })
}

// fn using_serve_dir_with_assets_fallback() -> Router {
//     // `ServeDir` allows setting a fallback if an asset is not found
//     // so with this `GET /assets/doesnt-exist.jpg` will return `index.html`
//     // rather than a 404
//     let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

//     Router::new()
//         .route("/foo", get(|| async { "Hi from /foo" }))
//         .nest_service("/assets", serve_dir.clone())
//         .fallback_service(serve_dir)
// }

// fn using_serve_dir_only_from_root_via_fallback() -> Router {
//     // you can also serve the assets directly from the root (not nested under `/assets`)
//     // by only setting a `ServeDir` as the fallback
//     let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

//     Router::new()
//         .route("/foo", get(|| async { "Hi from /foo" }))
//         .fallback_service(serve_dir)
// }

// fn using_serve_dir_with_handler_as_service() -> Router {
//     async fn handle_404() -> (StatusCode, &'static str) {
//         (StatusCode::NOT_FOUND, "Not found")
//     }

//     // you can convert handler function to service
//     let service = handle_404.into_service();

//     let serve_dir = ServeDir::new("assets").not_found_service(service);

//     Router::new()
//         .route("/foo", get(|| async { "Hi from /foo" }))
//         .fallback_service(serve_dir)
// }

// fn two_serve_dirs() -> Router {
//     // you can also have two `ServeDir`s nested at different paths
//     let serve_dir_from_assets = ServeDir::new("assets");
//     let serve_dir_from_dist = ServeDir::new("dist");

//     Router::new()
//         .nest_service("/assets", serve_dir_from_assets)
//         .nest_service("/dist", serve_dir_from_dist)
// }

// #[allow(clippy::let_and_return)]
// fn calling_serve_dir_from_a_handler() -> Router {
//     // via `tower::Service::call`, or more conveniently `tower::ServiceExt::oneshot` you can
//     // call `ServeDir` yourself from a handler
//     Router::new().nest_service(
//         "/foo",
//         get(|request: Request| async {
//             let service = ServeDir::new("assets");
//             let result = service.oneshot(request).await;
//             result
//         }),
//     )
// }

// fn using_serve_file_from_a_route() -> Router {
//     Router::new().route_service("/foo", ServeFile::new("assets/index.html"))
// }

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}