/// Helper macro to check that an attribute has been completely consumed.
macro_rules! check_attributes {
    ($diag:expr, $attr:expr) => {{
        for unused in $attr.unused() {
            $diag.err(unused, "unknown attribute");
        }

        if $diag.has_errors() {
            return Err(());
        }
    }};
}

/// Helper macro to check that a selection has been completely consumed.
macro_rules! check_selection {
    ($diag:expr, $sel:expr) => {{
        for unused in $sel.unused() {
            $diag.err(unused, "unknown attribute");
        }

        if $diag.has_errors() {
            return Err(());
        }
    }};
}

mod attributes;
mod features;
mod into_model;
mod scope;
pub mod session;
pub mod translated;

pub use self::session::{Packages, Session};
pub use self::translated::Translated;
