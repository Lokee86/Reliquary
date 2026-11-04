//! Durable owner-local Episode work claims and authenticated completion receipts.
//! Episode admission and completion truth are supplied by the higher owner layer.
mod store;
pub use store::*;
#[cfg(test)]
mod tests;
