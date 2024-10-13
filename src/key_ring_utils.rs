use std::{collections::HashMap, fs::{self, File, Permissions}, io::{Read, Write}};
use fs2::FileExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
const PASSWORD_FOLDER: &str = "whisper_transcribe_rs";
const PASSWORD_FILE: &str = ".db_password.json";

pub fn set_password(key: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
    let password_folder = dirs::config_local_dir().unwrap().join(PASSWORD_FOLDER);
    let password_file = password_folder.join(PASSWORD_FILE);
    fs::create_dir_all(password_folder)?;
    let mut file = File::create(&password_file)?;
    FileExt::try_lock_exclusive(&file)?;
    #[cfg(unix)]
    {
        // Set file permissions to 0600
        let permissions = Permissions::from_mode(0o600);
        file.set_permissions(permissions)?;
    }
    let mut password_map: HashMap<String,String>;
    if password_file.exists() && password_file.metadata()?.len() > 0 {
        password_map = serde_json::from_str(&fs::read_to_string(&password_file)?)?;
    } else {
        password_map = HashMap::new();
    }
    password_map.insert(key.to_string(),password.to_string());
    file.write_all(serde_json::to_string(&password_map)?.as_bytes())?;
    FileExt::unlock(&file)?;
    Ok(())
}

pub fn get_password(key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let password_folder = dirs::config_local_dir().unwrap().join(PASSWORD_FOLDER);
    let password_file = password_folder.join(PASSWORD_FILE);
    let mut file = File::open(&password_file)?;
    FileExt::try_lock_exclusive(&file)?;
    let mut str1 = String::new();
    file.read_to_string(&mut str1)?;
    let password_map: HashMap<String,String> = serde_json::from_str(&str1)?;
    FileExt::unlock(&file)?;
    Ok(password_map.get(key).unwrap().to_string())
}