//! macOS and iOS.
//!
//! Apple's Standard Directory guidelines put every one of these at a fixed
//! location under the home directory, so no system API call is needed.

use crate::{home, UserDir, UserDirs};
use std::path::PathBuf;

fn suffix(which: UserDir) -> Option<&'static str> {
    Some(match which {
        UserDir::Audio => "Music",
        UserDir::Desktop => "Desktop",
        UserDir::Document => "Documents",
        UserDir::Download => "Downloads",
        UserDir::Font => "Library/Fonts",
        UserDir::Picture => "Pictures",
        UserDir::Public => "Public",
        UserDir::Video => "Movies",
        // macOS has neither a per-user template nor a projects directory.
        UserDir::Template | UserDir::Project => return None,
    })
}

pub(crate) fn resolve(which: UserDir) -> Option<PathBuf> {
    Some(home()?.join(suffix(which)?))
}

pub(crate) fn resolve_all() -> Option<UserDirs> {
    let home = home()?;
    let mut dirs: [Option<PathBuf>; crate::COUNT] = Default::default();
    for which in UserDir::ALL {
        dirs[which.index()] = suffix(which).map(|s| home.join(s));
    }
    Some(UserDirs { dirs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_dir_is_absent() {
        assert_eq!(suffix(UserDir::Template), None);
        assert_eq!(resolve(UserDir::Template), None);
    }

    #[test]
    fn video_dir_is_movies_not_videos() {
        assert_eq!(suffix(UserDir::Video), Some("Movies"));
    }

    #[test]
    fn fonts_live_under_library() {
        assert_eq!(suffix(UserDir::Font), Some("Library/Fonts"));
    }

    #[test]
    fn resolve_all_agrees_with_resolve() {
        let all = resolve_all().expect("home directory should be resolvable in tests");
        for which in UserDir::ALL {
            assert_eq!(
                all.get(which).map(PathBuf::from),
                resolve(which),
                "{which:?}"
            );
        }
    }

    #[test]
    fn paths_are_absolute_and_under_home() {
        let home = home().expect("home directory should be resolvable in tests");
        for which in UserDir::ALL {
            if let Some(path) = resolve(which) {
                assert!(path.is_absolute(), "{which:?} -> {path:?}");
                assert!(path.starts_with(&home), "{which:?} -> {path:?}");
            }
        }
    }
}
