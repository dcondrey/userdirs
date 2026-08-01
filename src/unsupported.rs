//! Targets with no user-directory concept.
//!
//! Reached on anything that is neither Unix nor Windows — `wasm32-unknown-unknown`
//! most commonly. Every lookup returns [`None`] rather than failing to compile,
//! so a cross-platform crate can depend on `userdirs` unconditionally and let
//! the wasm build fall through to whatever it does when a directory is absent.

use crate::{UserDir, UserDirs};
use std::path::PathBuf;

pub(crate) fn resolve(_which: UserDir) -> Option<PathBuf> {
    None
}

pub(crate) fn resolve_all() -> Option<UserDirs> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lookup_is_absent() {
        for which in UserDir::ALL {
            assert_eq!(resolve(which), None, "{which:?}");
        }
        assert!(resolve_all().is_none());
    }
}
