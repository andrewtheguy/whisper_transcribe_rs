use log::debug;
use reqwest::blocking::get;
use sha1::{Sha1, Digest};
use url::Url;


use std::{fs, io, path::{Path, PathBuf}};

pub fn get_filename_from_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {

    // Parse the URL
    let parsed_url = Url::parse(url)?;
    let path = parsed_url.path();
    if let Some(filename) = Path::new(path).file_name(){
        Ok(filename.to_str().unwrap().to_string())
    }else{
        Err("Failed to get filename from URL".into())
    }
}

pub fn download_to_temp_and_move(url: &str, destination: &str) -> Result<(), Box<dyn std::error::Error>> {

    // Create a temporary file. This file will be automatically deleted when dropped.
    let mut temp_file = tempfile::NamedTempFile::new()?;

    // Download the file
    let mut response = get(url)?;

    if response.status().is_success() {
        io::copy(&mut response, &mut temp_file)?;
        //while let Some(chunk) = response.chunk().await? {
        //    temp_file.write_all(&chunk)?;
        //}
        //// Move the temp file to the destination only if the download was successful.
        temp_file.persist(destination)?; // Moves the file to the final destination
        debug!("File downloaded and moved to: {}", destination);
    } else {
        return Err(format!("Failed to download the file. Status: {}", response.status()).into());
    }

    Ok(())
}

pub fn get_silero_model() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let v4_download_url = "https://github.com/snakers4/silero-vad/raw/refs/tags/v4.0/files/silero_vad.onnx";

    //let v5_download_url = "https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad.onnx";

    //let half = "https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad_half.onnx";

    let download_url = v4_download_url;

    // Create a Sha1 object
    let mut hasher = Sha1::new();

    // Write the input string to the hasher
    hasher.update(download_url);

    // Get the resulting hash as a hexadecimal string
    let result = hasher.finalize();

    // Convert the hash to a hex string
    let hex_output = format!("{:x}", result);

    let model_local_directory = dirs::cache_dir().unwrap().join(hex_output).join("whisper_transcribe_rs");
    fs::create_dir_all(&model_local_directory)?;
    let file_name = get_filename_from_url(download_url)?;
    let model_path = model_local_directory.join(file_name);
    if !model_path.exists() {
        debug!("Downloading model from {} to {}", download_url, model_path.to_str().unwrap());
        download_to_temp_and_move(download_url, model_path.to_str().unwrap())?;
    }
    Ok(model_path)
}

pub fn get_whisper_model(download_url: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {

    let model_local_directory = dirs::cache_dir().unwrap().join("whisper_transcribe_rs");
    fs::create_dir_all(&model_local_directory)?;
    let file_name = get_filename_from_url(download_url)?;
    let model_path = model_local_directory.join(file_name);
    if !model_path.exists() {
        debug!("Downloading model from {} to {}", download_url, model_path.to_str().unwrap());
        download_to_temp_and_move(download_url, model_path.to_str().unwrap())?;
    }
    Ok(model_path)
}