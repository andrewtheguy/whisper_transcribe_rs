use std::{fs, path::PathBuf};


pub fn get_config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = dirs::config_local_dir().unwrap().join("whisper_transcribe_rs_config");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}