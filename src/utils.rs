
pub fn build_current_thread_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rt)
}