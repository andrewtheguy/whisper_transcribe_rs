use std::sync::{Arc, LazyLock};

// n.b. static items do not call [`Drop`] on program termination, so this won't be deallocated.
// this is fine, as the OS can deallocate the terminated program faster than we can free memory
// but tools like valgrind might report "memory leaks" as it isn't obvious this is intentional.
static CURRENT_THREAD_RUNTIME: LazyLock<Arc<tokio::runtime::Runtime>> = LazyLock::new(|| {
    let rt = build_current_thread_runtime().unwrap();
    Arc::new(rt)
});

fn build_current_thread_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rt)
}

pub fn get_runtime() -> Arc<tokio::runtime::Runtime> {
    let rt: &Arc<tokio::runtime::Runtime> = &*CURRENT_THREAD_RUNTIME;
    Arc::clone(rt)
}