//! Run with
//!
//! ```not_rust
//! cargo run -p example-static-file-server
//! ```

use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response}, Json,
    routing::{get, Router},
  };
use log::info;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};



// We use static route matchers ("/" and "/index.html") to serve our home
// page.
async fn index_handler() -> impl IntoResponse {
    static_handler("/index.html".parse::<Uri>().unwrap()).await
}

// We use a wildcard matcher ("/dist/*file") to match against everything
// within our defined assets directory. This is the directory on our Asset
// struct below, where folder = "examples/public/".
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();

    // if path.starts_with("dist/") {
    //   path = path.replace("dist/", "");
    // }

    StaticFile(path)
}

// Finally, we use a fallback route for anything that didn't match.
async fn not_found() -> Html<&'static str> {
    Html("<h1>404</h1><p>Not Found</p>")
}

#[derive(Embed)]
#[folder = "./frontend/dist/"]
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
  T: Into<String>,
{
  fn into_response(self) -> Response {
    let path = self.0.into();

    match Asset::get(path.as_str()) {
      Some(content) => {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
      }
      None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
  }
}

pub async fn start_webserver(port: u16) {

    // Define our app routes, including a fallback option for anything not matched.
    let app = Router::new()
      .route("/", get(index_handler))
      .route("/index.html", get(index_handler))
      .route("/vite.svg", get(static_handler))
      .route("/assets/*file", get(static_handler))
      .route("/api/test", get(test_api))
      .fallback_service(get(not_found));

        serve(app, port).await;

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

// fn using_serve_dir() -> Router {

//     // serve the file in the "dist" directory under `/`
//     Router::new()
//     .route("/api/test", get(test_api))
//     .nest_service("/", ServeDir::new("frontend/dist"))
// }

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
    eprintln!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}