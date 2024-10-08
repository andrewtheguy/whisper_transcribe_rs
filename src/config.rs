use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
    pub database_file_path: Option<String>,
    pub language: String,
    pub operation: String, //should be enum
   //port: Option<u16>,
   //keys: Keys,
}

// #[derive(Deserialize)]
// struct Keys {
//    github: String,
//    travis: Option<String>,
// }
