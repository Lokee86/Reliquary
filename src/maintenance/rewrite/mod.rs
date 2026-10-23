//! Existing explicit fresh-history rewrite policy and emission.
mod error;
pub use error::RewriteError;
pub(crate) mod archive_selection;
pub(crate) mod closure;
pub(crate) mod model;
mod plan;
pub(crate) mod plan_source;
pub(crate) use plan_source::{PhylacteryRewritePlanSource, ReliquaryRewritePlanSource};

pub(crate) mod semantic_emit;

pub(crate) mod archive_emit;

mod file;
pub(crate) mod owner_access;
pub use file::{plan_rewrite_file, rewrite_file, rewrite_from_plan};
pub use model::*;
