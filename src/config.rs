use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
    pub database_file_path: Option<String>,
    pub vad_onnx_model_path: String,
    pub whisper_model_path: String,
    pub language: String,
   //port: Option<u16>,
   //keys: Keys,
}

// #[derive(Deserialize)]
// struct Keys {
//    github: String,
//    travis: Option<String>,
// }
