//! Run with
//!
//! ```not_rust
//! cargo run -p example-static-file-server
//! ```

use axum::{
  http::{header, HeaderValue, StatusCode, Uri}, response::{Html, IntoResponse, Response}, routing::{get, Router}, Json
};
use chrono::{DateTime, FixedOffset, Utc};
use crossbeam::channel::Sender;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres, Row};
use std::net::SocketAddr;

use tower_http::trace::TraceLayer;

use crate::{streaming::Segment, vad_processor::SAMPLE_SIZE};


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


#[axum::debug_handler]
async fn set_session_id(axum::extract::State(state): axum::extract::State<AppState>,input: Json<SessionIdInput>) -> Json<TestResponse> {
  //eprintln!("session id: {}", input.id);
  let mut session_id = state.session_id.lock().await;
  *session_id = Some(input.id.clone());
  Json(TestResponse {
    message: "success".to_string(),
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

fn parse_timestamp_millis(ts: &HeaderValue) -> Result<i64, Box<dyn std::error::Error>> {
  Ok(ts.to_str()?.parse::<i64>()?)
}

fn parse_session_id(session_id_header: &HeaderValue) -> Result<&str, Box<dyn std::error::Error>> {
  Ok(session_id_header.to_str()?)
}

#[axum::debug_handler]
async fn audio_input(axum::extract::State(state): axum::extract::State<AppState>,request: axum::http::Request<axum::body::Body>) -> Result<Json<TestResponse>, impl IntoResponse> {
  
  let timestamp_millis = if let Some(ts)= request.headers().get("X-Recording-Timestamp") {
    match parse_timestamp_millis(ts) {
      Ok(ts) => ts,
      Err(e) => {
        return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
          message: format!("error parsing timestamp: {}", e),
        })));
      }
    }
  } else {
    return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
      message: "missing timestamp".to_string(),
    })));
  };
  
  let session_id_option = state.session_id.lock().await;
  
  
  let current_session_id = match session_id_option.clone() {
    Some(session_id) => session_id,
    None => {
      return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
        message: "session id is not set yet".to_string(),
      }))); 
    }
  };
  
  drop(session_id_option);
  
  let session_id = if let Some(session_id_header) = request.headers().get("X-Session-Id") {
    match parse_session_id(session_id_header) {
      Ok(session_id) => session_id,
      Err(e) => {
        return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
          message: format!("error parsing session id: {}", e),
        })));
      }
    }
  } else {
    return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
      message: "missing session id".to_string(),
    })));
  };
  
  if session_id != current_session_id {
    return Err((StatusCode::BAD_REQUEST,Json(TestResponse {
      message: "session id mismatch, set a new session id if you intent to start a new session and close previous sessions".to_string(),
    })));
  }
  
  let body_stream = request.into_body().into_data_stream();
  
  match read_stream(body_stream, &state.tx, timestamp_millis).await {
    Ok(_) => {},
    Err(e) => {
      return Err((StatusCode::INTERNAL_SERVER_ERROR,Json(TestResponse {
        message: format!("error reading stream: {}", e),
      })));
    }
  };
  
  Ok(Json(TestResponse {
    message: "success".to_string(),
  }))
}

#[derive(Deserialize)]
struct TranscriptQuery {
  before_id: Option<i64>,
  after_id: Option<i64>,
  show_name: String,
}

#[axum::debug_handler]
async fn get_transcripts(state: axum::extract::State<AppState>,q: axum::extract::Query<TranscriptQuery>) -> impl IntoResponse {
  let query;
  if let Some (before_id) = q.before_id {
    let row = sqlx::query(r#"SELECT id FROM transcripts where show_name = $1 and id < $2 order by id desc limit 1 offset 100"#)
    .bind(q.show_name.clone())
    .bind(before_id)
    .fetch_optional(&state.pool).await.unwrap();

    let after_id_2: i64 =if let Some(row) = row {
       row.try_get("id").unwrap()
    } else {
      i64::MAX
    };

    
    
    let pool = &state.pool;
    query = sqlx::query(r#"SELECT id,"timestamp",content FROM transcripts where show_name = $1 and id >= $2 and id < $3 order by id limit 1000"#)
    .bind(q.show_name.clone())
    .bind(after_id_2)
    .bind(before_id)
    .fetch(pool);
  } else {
    let mut after_id = q.after_id.unwrap_or(0);
    
    if after_id == 0 {
      let row = sqlx::query(r#"SELECT id FROM transcripts where show_name = $1 order by id desc limit 1 offset 100"#)
      .bind(q.show_name.clone())
      .fetch_optional(&state.pool).await.unwrap();
      if let Some(row) = row {
        after_id = row.try_get("id").unwrap();
      } else {
        after_id = 0;
      }
    }

    let pool = &state.pool;
    query = sqlx::query(r#"SELECT id,"timestamp",content FROM transcripts where show_name = $1 and id > $2 order by id limit 1000"#)
    .bind(q.show_name.clone())
    .bind(after_id)
    .fetch(pool);
  }
  
  
  let rows_stream = query.map(|row| {
    let row = row.unwrap();
    let id: i64 = row.try_get("id").unwrap();
    let timestamp: chrono::NaiveDateTime = row.try_get("timestamp").unwrap();
    let content: String = row.try_get("content").unwrap();
    let test =timestamp.and_utc().to_rfc3339();
    json!({"id": id, "timestamp": test, "content": content}).to_string()
  })
  .map(|json_row| Ok::<String, std::convert::Infallible>(format!("{}\n",json_row).into()));
  
  // Create the Axum body from the stream
  let body = axum::body::Body::from_stream(rows_stream);
  
  (StatusCode::OK,body).into_response()
}

async fn get_show_names(state: axum::extract::State<AppState>) -> impl IntoResponse {
  let pool = &state.pool;
  let mut rows = sqlx::query(r#"SELECT distinct show_name FROM transcripts"#)
  .fetch(pool);
  let mut show_names = Vec::new();
  while let Some(row) = rows.next().await {
    let row = row.unwrap();
    let show_name: String = row.try_get("show_name").unwrap();
    show_names.push(show_name);
  }
  Json(show_names)
}

#[derive(Serialize, Deserialize)]
struct TestResponse {
  message: String,
}


#[derive(Serialize, Deserialize)]
struct SessionIdInput {
  id: String,
}

#[derive(Clone)]
struct AppState {
  tx: Sender::<Option<Segment>>,
  pool: Pool<Postgres>,
  session_id: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
}

pub struct TranscribeWebServer {
  port: u16,
  state: AppState,
}

impl TranscribeWebServer {
  pub fn new(port: u16, tx: Sender::<Option<Segment>>, pool: Pool<Postgres>) -> Self {
    Self {
      port,
      state: AppState {
        tx,
        session_id: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        pool,
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
    .route("/api/get_transcripts", axum::routing::get(get_transcripts))
    .route("/api/get_show_names", axum::routing::get(get_show_names))
    
    .route("/api/set_session_id", axum::routing::post(set_session_id))
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