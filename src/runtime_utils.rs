use std::sync::LazyLock;

// n.b. static items do not call [`Drop`] on program termination, so this won't be deallocated.
// this is fine, as the OS can deallocate the terminated program faster than we can free memory
// but tools like valgrind might report "memory leaks" as it isn't obvious this is intentional.
static CURRENT_THREAD_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    let rt = build_current_thread_runtime().unwrap();
    rt
});

fn build_current_thread_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rt)
}

pub fn get_runtime() -> &'static tokio::runtime::Runtime {
    &*CURRENT_THREAD_RUNTIME
}