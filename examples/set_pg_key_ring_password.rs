//use rpassword::read_password;
use std::io::Write;

use whisper_transcribe_rs::key_ring_utils;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print!("Type the password: ");
    std::io::stdout().flush().unwrap();
    let mut buf: String = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let password = buf.trim();
    //let password = entry.get_password()?;
    println!("My password is '{}'", password);
    key_ring_utils::set_password("postgres_portainer_instance_password", password)?;
    Ok(())
}