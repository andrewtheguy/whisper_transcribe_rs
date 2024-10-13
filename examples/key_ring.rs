use keyring::{Entry, Result};

fn main() -> Result<()> {
    let entry = Entry::new("my-service", "my-name")?;
    //entry.set_password("test_password")?;
    let password = entry.get_password()?;
    println!("My password is '{}'", password);
    Ok(())
}