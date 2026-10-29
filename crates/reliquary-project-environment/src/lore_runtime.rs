use lore::interface::LoreGlobalArgs;
use std::path::Path;

pub(crate) fn globals(root: &Path) -> LoreGlobalArgs {
    LoreGlobalArgs {
        repository_path: root.to_string_lossy().as_ref().into(),
        offline: 1,
        local: 1,
        ..Default::default()
    }
}

pub(crate) fn block_on_lore<F>(future: F) -> F::Output
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::scope(|scope| {
            scope
                .spawn(move || lore::runtime().block_on(future))
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    } else {
        lore::runtime().block_on(future)
    }
}
