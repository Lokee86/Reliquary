mod receipt;
mod report;
mod validate;

pub(crate) const MAX_CHAIN_PAGES: usize = 4096;
pub(crate) use receipt::{PageReceipt, PageSnapshot};
pub use validate::validate_memory_page_chain;
