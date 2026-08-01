//! Cross-platform paths to the user's media and personal directories.
//!
//! This crate covers the directories a *person* keeps files in — Downloads,
//! Documents, Pictures, Music — as opposed to the directories an *application*
//! keeps state in. For config/data/cache/state/runtime, use
//! [`etcetera`](https://docs.rs/etcetera), which deliberately scopes itself to
//! those and leaves this set uncovered.
//!
//! ```no_run
//! if let Some(dir) = userdirs::download_dir() {
//!     println!("downloads live at {}", dir.display());
//! }
//! ```
//!
//! # Platform behaviour
//!
//! | function | Linux / BSD | macOS | Windows |
//! |---|---|---|---|
//! | [`audio_dir`] | `XDG_MUSIC_DIR` | `$HOME/Music` | `FOLDERID_Music` |
//! | [`desktop_dir`] | `XDG_DESKTOP_DIR` | `$HOME/Desktop` | `FOLDERID_Desktop` |
//! | [`document_dir`] | `XDG_DOCUMENTS_DIR` | `$HOME/Documents` | `FOLDERID_Documents` |
//! | [`download_dir`] | `XDG_DOWNLOAD_DIR` | `$HOME/Downloads` | `FOLDERID_Downloads` |
//! | [`font_dir`] | `$XDG_DATA_HOME/fonts` | `$HOME/Library/Fonts` | — |
//! | [`picture_dir`] | `XDG_PICTURES_DIR` | `$HOME/Pictures` | `FOLDERID_Pictures` |
//! | [`project_dir`] | `XDG_PROJECTS_DIR` | — | — |
//! | [`public_dir`] | `XDG_PUBLICSHARE_DIR` | `$HOME/Public` | `FOLDERID_Public` |
//! | [`template_dir`] | `XDG_TEMPLATES_DIR` | — | `FOLDERID_Templates` |
//! | [`video_dir`] | `XDG_VIDEOS_DIR` | `$HOME/Movies` | `FOLDERID_Videos` |
//!
//! Every function returns [`None`] when the platform has no such concept, or
//! when the home directory cannot be determined. A returned path is **not**
//! guaranteed to exist on disk.
//!
//! Targets that are neither Unix nor Windows — `wasm32-unknown-unknown` in
//! particular — compile and return [`None`] from every function, so depending
//! on this crate does not break a build matrix that includes one.
//!
//! On Linux the XDG keys are read from `${XDG_CONFIG_HOME:-$HOME/.config}/user-dirs.dirs`.
//! Each call re-reads that file; use [`UserDirs::new`] to read it once and reuse
//! the result.

#![deny(missing_docs)]
#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
// Anything that is neither Unix nor Windows, so the crate stays buildable on
// wasm and other exotic targets instead of breaking every dependent that has
// one in its matrix.
#[cfg(not(any(unix, windows)))]
mod unsupported;
#[cfg(windows)]
mod windows;

// Compiled on every platform, not only XDG ones, so that the `user-dirs.dirs`
// parser — the only non-trivial logic in the crate — is exercised by `cargo
// test` wherever it runs, rather than solely in Linux CI.
mod xdg;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use apple as imp;
#[cfg(not(any(unix, windows)))]
use unsupported as imp;
#[cfg(windows)]
use windows as imp;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
use xdg as imp;

/// Which user directory to resolve.
///
/// Used internally to keep the free functions from duplicating the platform
/// dispatch. Public so [`UserDirs::get`] can accept it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UserDir {
    /// Music and other audio.
    Audio,
    /// The desktop.
    Desktop,
    /// Documents.
    Document,
    /// Downloads.
    Download,
    /// User-installed fonts.
    Font,
    /// Images.
    Picture,
    /// The user's projects.
    Project,
    /// Files shared with other users of the machine.
    Public,
    /// Document templates.
    Template,
    /// Movies and other video.
    Video,
}

macro_rules! accessors {
    ($($(#[$m:meta])* $name:ident => $variant:ident),* $(,)?) => {
        $(
            $(#[$m])*
            pub fn $name() -> Option<PathBuf> {
                imp::resolve(UserDir::$variant)
            }
        )*

        impl UserDirs {
            $(
                $(#[$m])*
                pub fn $name(&self) -> Option<&std::path::Path> {
                    self.get(UserDir::$variant)
                }
            )*
        }
    };
}

accessors! {
    /// Path to the user's audio directory.
    audio_dir => Audio,
    /// Path to the user's desktop directory.
    desktop_dir => Desktop,
    /// Path to the user's document directory.
    document_dir => Document,
    /// Path to the user's download directory.
    download_dir => Download,
    /// Path to the user's font directory.
    ///
    /// Returns [`None`] on Windows, which has no per-user font directory
    /// exposed through the Known Folder API.
    font_dir => Font,
    /// Path to the user's picture directory.
    picture_dir => Picture,
    /// Path to the user's projects directory.
    ///
    /// Returns [`None`] outside Linux/BSD. `XDG_PROJECTS_DIR` ships in the
    /// `xdg-user-dirs` defaults but has no macOS or Windows counterpart.
    project_dir => Project,
    /// Path to the user's public share directory.
    public_dir => Public,
    /// Path to the user's template directory.
    ///
    /// Returns [`None`] on macOS, which has no per-user template directory.
    template_dir => Template,
    /// Path to the user's video directory.
    video_dir => Video,
}

/// All user directories, resolved once.
///
/// On Linux the free functions each re-parse `user-dirs.dirs`. Constructing a
/// `UserDirs` reads that file a single time, which matters if you need more
/// than one directory.
///
/// Deliberately not `Default`: an all-`None` `UserDirs` is indistinguishable
/// from a successfully resolved one at the type level, so the only way to build
/// one is [`UserDirs::new`].
#[derive(Debug, Clone)]
pub struct UserDirs {
    dirs: [Option<PathBuf>; COUNT],
}

impl UserDirs {
    /// Resolve every user directory.
    ///
    /// Returns [`None`] only if the home directory cannot be determined at all.
    /// Individual entries may still be [`None`] on platforms lacking them.
    pub fn new() -> Option<Self> {
        imp::resolve_all()
    }

    /// Path to one user directory, or [`None`] if this platform lacks it.
    pub fn get(&self, which: UserDir) -> Option<&std::path::Path> {
        self.dirs[which.index()].as_deref()
    }
}

/// Number of [`UserDir`] variants. Every per-directory array is this wide, so
/// adding a variant cannot leave a stale length behind.
pub(crate) const COUNT: usize = 10;

impl UserDir {
    pub(crate) const ALL: [UserDir; COUNT] = [
        UserDir::Audio,
        UserDir::Desktop,
        UserDir::Document,
        UserDir::Download,
        UserDir::Font,
        UserDir::Picture,
        UserDir::Project,
        UserDir::Public,
        UserDir::Template,
        UserDir::Video,
    ];

    pub(crate) const fn index(self) -> usize {
        match self {
            UserDir::Audio => 0,
            UserDir::Desktop => 1,
            UserDir::Document => 2,
            UserDir::Download => 3,
            UserDir::Font => 4,
            UserDir::Picture => 5,
            UserDir::Project => 6,
            UserDir::Public => 7,
            UserDir::Template => 8,
            UserDir::Video => 9,
        }
    }
}

pub(crate) fn home() -> Option<PathBuf> {
    // Un-deprecated and corrected in Rust 1.85; this is why the crate needs no
    // `libc` dependency for a `getpwuid_r` fallback. It prefers `$HOME` when
    // set, so paths expand against the same home directory the rest of the
    // desktop resolves `user-dirs.dirs` against.
    std::env::home_dir().filter(|h| !h.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_unique_index_in_range() {
        // `ALL` and `index` are maintained by hand. If they ever disagree, the
        // per-directory arrays silently return the wrong path or panic, so the
        // mapping is asserted to be a bijection onto `0..COUNT`.
        let mut seen = [false; COUNT];
        for which in UserDir::ALL {
            let index = which.index();
            assert!(index < COUNT, "{which:?} has out-of-range index {index}");
            assert!(!seen[index], "{which:?} reuses index {index}");
            seen[index] = true;
        }
        assert!(
            seen.iter().all(|slot| *slot),
            "UserDir::ALL does not cover every index in 0..{COUNT}"
        );
    }
}
