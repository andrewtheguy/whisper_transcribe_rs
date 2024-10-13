//use rpassword::read_password;
use std::{env::args, fs, io::Write};

use whisper_transcribe_rs::{config::Config, key_ring_utils};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_file = args().nth(1).expect("No config file provided");
    let config: Config = toml::from_str(fs::read_to_string(config_file)?.as_str()).unwrap();
    let database_password_key = config.database_config.unwrap().database_password_key.clone();

    print!("Type the password for database_password_key {}: ", database_password_key);
    std::io::stdout().flush().unwrap();
    let mut buf: String = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let password = buf.trim();
    //let password = entry.get_password()?;
    println!("My password is '{}'", password);
    key_ring_utils::set_password(&database_password_key, password)?;
    Ok(())
}