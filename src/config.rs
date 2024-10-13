use serde::Deserialize;

#[derive(Deserialize)]
pub struct DatabaseConfig {
    pub database_host: String,
    pub database_port: Option<u16>,
    pub database_user: String,
    pub database_password_key: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
    pub database_config: Option<DatabaseConfig>,
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
