//use rpassword::read_password;
use std::io::Write;
use keyring::{Entry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print!("Type the password: ");
    std::io::stdout().flush().unwrap();
    let mut buf: String = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let entry = Entry::new("whisper_transcribe_rs", "postgres_password")?;
    let password = buf.trim();
    entry.set_password(password)?;
    //let password = entry.get_password()?;
    println!("My password is '{}'", password);
    Ok(())
}