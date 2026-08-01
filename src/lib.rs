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
//! | [`font_dir`] | `$XDG_DATA_HOME/fonts` | `$HOME/Library/Fonts` | `%LOCALAPPDATA%\Microsoft\Windows\Fonts` |
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
    /// On Windows this is `%LOCALAPPDATA%\Microsoft\Windows\Fonts`, where a
    /// non-elevated font install writes as of Windows 10 1803. It is
    /// deliberately not `FOLDERID_Fonts`, which is documented as FIXED at
    /// `%windir%\Fonts` and is the machine-wide store rather than a user
    /// directory.
    font_dir => Font,
    /// Path to the user's picture directory.
    picture_dir => Picture,
    /// Path to the user's projects directory.
    ///
    /// Returns [`None`] outside Linux/BSD. `XDG_PROJECTS_DIR` ships in the
    /// `xdg-user-dirs` defaults but has no macOS or Windows counterpart.
    project_dir => Project,
    /// Path to the public share directory.
    ///
    /// Per-user on Unix (`$HOME/Public`), but machine-wide on Windows:
    /// `FOLDERID_Public` is documented as FIXED at `%PUBLIC%`
    /// (`%SystemDrive%\Users\Public`) and is shared by every account.
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
/// On Linux the free functions each re-read and re-parse `user-dirs.dirs`.
/// Constructing a `UserDirs` does that once, so fetching *n* directories costs
/// one file read instead of *n*.
///
/// That is the whole of the difference worth caring about: reading the file
/// costs roughly fourteen times what parsing it does, so the saving is in the
/// syscalls, not the parsing. Reach for this when you need more than one
/// directory; a single lookup gains nothing from it.
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
    /// Every directory this crate knows about.
    ///
    /// Returns an iterator rather than an array so that adding a variant stays
    /// a non-breaking change; the count is not part of the signature.
    ///
    /// ```
    /// # use userdirs::{UserDir, UserDirs};
    /// if let Some(dirs) = UserDirs::new() {
    ///     for which in UserDir::all() {
    ///         println!("{which:?}: {:?}", dirs.get(which));
    ///     }
    /// }
    /// ```
    pub fn all() -> impl Iterator<Item = UserDir> {
        Self::ALL.into_iter()
    }

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

/// A process-wide cached snapshot of every user directory.
///
/// Resolving costs a `user-dirs.dirs` read on Linux and one shell call per
/// directory on Windows. Where those paths are consulted repeatedly — a
/// long-running process, a hot path, a file dialog opened over and over — this
/// pays for the lookup once.
///
/// # This is opt-in twice, deliberately
///
/// Turning on the `cache` feature adds this module and changes nothing else.
/// The free functions and [`UserDirs::new`] keep reading from disk on every
/// call. That is on purpose: Cargo unifies features across a dependency graph,
/// so if enabling `cache` silently rewired [`download_dir`], any unrelated
/// crate could switch *your* lookups to cached values without you knowing.
/// Staleness has to be something you ask for at the call site.
///
/// # Staleness
///
/// The snapshot is taken once and reused. `xdg-user-dirs-update` can rewrite
/// `user-dirs.dirs` while the process runs, and a desktop environment can
/// relocate a folder at any time, so a long-lived cache will eventually be
/// wrong. Call [`cache::reload`] after any point where the directories may
/// have moved. GLib takes the same approach with
/// `g_reload_user_special_dirs_cache()`.
#[cfg(feature = "cache")]
pub mod cache {
    use super::UserDirs;
    use std::sync::{Arc, RwLock};

    /// `None` means "not yet resolved"; `Some(None)` means "resolved, and there
    /// are no directories", which must not trigger a re-resolve on every call.
    static CACHE: RwLock<Option<Option<Arc<UserDirs>>>> = RwLock::new(None);

    /// The cached snapshot, resolving it on first use.
    ///
    /// Returns [`None`] under the same conditions as [`UserDirs::new`]: the
    /// home directory could not be determined.
    pub fn user_dirs() -> Option<Arc<UserDirs>> {
        // A poisoned lock is not a reason to fail: fall through and resolve
        // uncached rather than panicking in a path-lookup function.
        if let Ok(guard) = CACHE.read() {
            if let Some(resolved) = guard.as_ref() {
                return resolved.clone();
            }
        }

        let resolved = UserDirs::new().map(Arc::new);

        if let Ok(mut guard) = CACHE.write() {
            // A racing thread may have filled this already. Both values are
            // equally valid, so last writer wins is fine.
            *guard = Some(resolved.clone());
        }

        resolved
    }

    /// Discard the snapshot so the next [`user_dirs`] call resolves again.
    ///
    /// Callers already holding an [`Arc`] keep the old snapshot; this only
    /// affects subsequent lookups.
    pub fn reload() {
        if let Ok(mut guard) = CACHE.write() {
            *guard = None;
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

#[cfg(all(test, feature = "cache"))]
mod cache_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn snapshot_is_reused_until_reloaded() {
        // Serialised by hand: the cache is process-wide, so these steps cannot
        // be split into separate #[test] fns without racing each other.
        cache::reload();

        let Some(first) = cache::user_dirs() else {
            // No home directory on this machine; nothing to assert.
            return;
        };
        let second = cache::user_dirs().expect("still resolvable");

        // Same allocation, so the second call did no work.
        assert!(
            Arc::ptr_eq(&first, &second),
            "second lookup should reuse the snapshot"
        );

        cache::reload();
        let third = cache::user_dirs().expect("still resolvable");
        assert!(
            !Arc::ptr_eq(&first, &third),
            "reload should force a fresh resolve"
        );

        // Invalidation must not change the answer, only recompute it.
        for which in UserDir::ALL {
            assert_eq!(first.get(which), third.get(which), "{which:?}");
        }
    }

    /// Run with `cargo test --all-features --release -- --ignored --nocapture cache_cost`.
    #[test]
    #[ignore = "timing only; run with --release"]
    fn cache_cost() {
        cache::reload();
        let _ = cache::user_dirs();

        let iters = 200_000;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            std::hint::black_box(cache::user_dirs());
        }
        println!("cache::user_dirs() (warm)  {:?}", start.elapsed() / iters);

        let start = std::time::Instant::now();
        for _ in 0..iters {
            std::hint::black_box(UserDirs::new());
        }
        println!("UserDirs::new() (uncached) {:?}", start.elapsed() / iters);
    }

    #[test]
    fn cached_values_match_the_uncached_api() {
        cache::reload();
        let Some(cached) = cache::user_dirs() else {
            return;
        };
        let direct = UserDirs::new().expect("resolvable");
        for which in UserDir::ALL {
            assert_eq!(cached.get(which), direct.get(which), "{which:?}");
        }
    }
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
