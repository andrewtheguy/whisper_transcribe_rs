use std::{fs, path::PathBuf};

pub fn build_current_thread_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rt)
}

pub fn get_config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = dirs::config_local_dir().unwrap().join("whisper_transcribe_rs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}