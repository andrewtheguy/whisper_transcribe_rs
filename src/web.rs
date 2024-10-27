//! Run with
//!
//! ```not_rust
//! cargo run -p example-static-file-server
//! ```

use axum::{
    http::{header, request, StatusCode, Uri}, response::{Html, IntoResponse, Response}, routing::{get, Router}, Json
  };
use crossbeam::channel::Sender;
use log::info;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufRead;
use std::net::SocketAddr;

use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{streaming::Segment, vad_processor::{save_buf_to_file, SAMPLE_SIZE}};

use tokio::io::{self, AsyncReadExt};

use futures::StreamExt;

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

struct StaticFile<T>(pub T);

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

#[axum::debug_handler]
async fn test_api(axum::extract::State(state): axum::extract::State<AppState>) -> Json<TestResponse> {

  Json(TestResponse {
      message: "hello test".to_string(),
  })
}

fn process_chunk(buffer: &[u8],tx: &Sender::<Option<Segment>>,timestamp_millis: i64) {

  // Convert the raw byte buffer into Vec<i16>
  let mut samples: Vec<i16> = Vec::with_capacity(buffer.len() / 2); // i16 is 2 bytes
  for chunk in buffer.chunks_exact(2) {
      let sample = i16::from_le_bytes(chunk.try_into().unwrap()); // Convert 2 bytes to i16
      samples.push(sample);
  }

  //eprintln!("timestamp_millis: {}", timestamp_millis);

  let segment = Segment {
    samples: samples,
    timestamp_millis: timestamp_millis,
  };
  //eprintln!("segment len: {}", segment.samples.len());
  tx.send(Some(segment)).unwrap();
  //let file_name = std::path::PathBuf::from("tmp/test.wav");
  //save_buf_to_file(&segment.samples, &file_name)
}

async fn read_stream(mut body_stream: axum::body::BodyDataStream,tx: &Sender::<Option<Segment>>, timestamp_millis: i64) -> Result<(), Box<dyn std::error::Error>> {
  
  const CHUNK_SIZE: usize = SAMPLE_SIZE as usize * 2; // 2 bytes per sample

  let mut buffer = Vec::with_capacity(CHUNK_SIZE);

  while let Some(chunk) = body_stream.next().await {
      match chunk {
          Ok(data) => {
              buffer.extend_from_slice(&data);

              // Process in CHUNK_SIZE portions if the buffer has enough data
              while buffer.len() >= CHUNK_SIZE {
                  let chunk_to_process = buffer.drain(..CHUNK_SIZE).collect::<Vec<u8>>();
                  process_chunk(&chunk_to_process,&tx,timestamp_millis);
              }
          }
          Err(e) => {
              eprintln!("Error reading body chunk: {:?}", e);
              return Err(e.into());
          }
      }
  }

  // Process any remaining data in the buffer
  if !buffer.is_empty() {
      process_chunk(&buffer,&tx,timestamp_millis);
  }

  //tx.send(None).unwrap();

  Ok(())
}

#[axum::debug_handler]
async fn audio_input(axum::extract::State(state): axum::extract::State<AppState>,request: axum::http::Request<axum::body::Body>) -> (StatusCode,Json<TestResponse>) {

  let timestamp_millis = if let Some(ts)= request.headers().get("X-Recording-Timestamp") {
     ts.to_str().unwrap().parse::<i64>().unwrap().clone()
  } else {
    return (StatusCode::BAD_REQUEST,Json(TestResponse {
      message: "missing timestamp".to_string(),
    }));
  };
  

  let body_stream = request.into_body().into_data_stream();

  read_stream(body_stream, &state.tx, timestamp_millis).await.unwrap();

  (StatusCode::OK,Json(TestResponse {
      message: "success".to_string(),
  }))
}

#[derive(Serialize, Deserialize)]
struct TestResponse {
    message: String,
}

#[derive(Clone)]
struct AppState {
  tx: Sender::<Option<Segment>>,
}

pub struct TranscribeWebServer {
    port: u16,
    state: AppState,
}

impl TranscribeWebServer {
    pub fn new(port: u16, tx: Sender::<Option<Segment>>) -> Self {
        Self {
            port,
            state: AppState {
                tx,
            }
        }
    }

    pub async fn start_webserver(self) {

      //let m = move || async move { self.test_api() };

      // Define our app routes, including a fallback option for anything not matched.
      let app = Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/vite.svg", get(static_handler))
        .route("/assets/*file", get(static_handler))
        .route("/api/test", axum::routing::get(test_api))
        .route("/api/audio_input", axum::routing::post(audio_input))
        .with_state(self.state.clone())
        .fallback_service(get(not_found));
  
        self.serve(app).await;
  
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

  async fn serve(self, app: Router) {
      let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
      let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
      eprintln!("listening on {}", listener.local_addr().unwrap());
      axum::serve(listener, app.layer(TraceLayer::new_for_http()))
          .await
          .unwrap();
  }

}