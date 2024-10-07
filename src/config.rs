use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    url: String,
    database_file_path: Option<String>,
    onnx_model_path: String,
    whisper_model_path: String,
    language: String,
   //port: Option<u16>,
   //keys: Keys,
}

// #[derive(Deserialize)]
// struct Keys {
//    github: String,
//    travis: Option<String>,
// }
