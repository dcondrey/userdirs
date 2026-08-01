//! Linux, BSD and other XDG platforms.
//!
//! Reads `${XDG_CONFIG_HOME:-$HOME/.config}/user-dirs.dirs`, the file written by
//! `xdg-user-dirs-update(1)`.
//!
//! Compiled on every platform so its parser tests always run; only XDG targets
//! actually call into it.
#![cfg_attr(
    not(all(unix, not(any(target_os = "macos", target_os = "ios")))),
    allow(dead_code)
)]

use crate::{home, UserDir, UserDirs};
use std::path::{Path, PathBuf};

fn key_for(which: UserDir) -> Option<&'static str> {
    Some(match which {
        UserDir::Audio => "MUSIC",
        UserDir::Desktop => "DESKTOP",
        UserDir::Document => "DOCUMENTS",
        UserDir::Download => "DOWNLOAD",
        UserDir::Picture => "PICTURES",
        UserDir::Project => "PROJECTS",
        UserDir::Public => "PUBLICSHARE",
        UserDir::Template => "TEMPLATES",
        UserDir::Video => "VIDEOS",
        // Not part of user-dirs.dirs; derived from XDG_DATA_HOME instead.
        UserDir::Font => return None,
    })
}

/// Resolve an XDG base directory from its environment variable.
///
/// The spec requires a relative value to be ignored as invalid, so the
/// env-reading and the rule are kept apart: the rule is testable, the env read
/// is not worth testing.
fn base_dir(var: Option<std::ffi::OsString>, home: &Path, fallback: &str) -> PathBuf {
    match var.filter(|v| !v.is_empty()) {
        Some(v) if Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => home.join(fallback),
    }
}

fn config_home(home: &Path) -> PathBuf {
    base_dir(std::env::var_os("XDG_CONFIG_HOME"), home, ".config")
}

fn data_home(home: &Path) -> PathBuf {
    base_dir(std::env::var_os("XDG_DATA_HOME"), home, ".local/share")
}

/// Trim leading and trailing ASCII whitespace, `\r` included so that CRLF line
/// endings do not leave a stray byte on the value.
fn trim(mut bytes: &[u8]) -> &[u8] {
    while let Some((first, rest)) = bytes.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        bytes = rest;
    }
    while let Some((last, rest)) = bytes.split_last() {
        if !last.is_ascii_whitespace() {
            break;
        }
        bytes = rest;
    }
    bytes
}

/// Build a path from raw bytes.
///
/// Unix paths are arbitrary bytes, not UTF-8, so the parser must never require
/// valid UTF-8 to produce a path.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// Non-Unix fallback. XDG lookup never runs on these targets; this exists only
/// so the parser and its tests still compile there.
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// Parse the body of a `user-dirs.dirs` file.
///
/// Takes bytes rather than `&str`: a single non-UTF-8 byte anywhere in the file
/// must not be able to discard every directory in it.
///
/// Split out from any I/O so it can be tested directly.
pub(crate) fn parse(contents: &[u8], home: &Path) -> [Option<PathBuf>; crate::COUNT] {
    let mut out: [Option<PathBuf>; crate::COUNT] = Default::default();

    for line in contents.split(|b| *b == b'\n') {
        let line = trim(line);
        if line.first() == Some(&b'#') {
            continue;
        }
        let Some(eq) = line.iter().position(|b| *b == b'=') else {
            continue;
        };

        // Keys are ASCII by construction; a non-UTF-8 key cannot be one of ours.
        let Ok(key) = std::str::from_utf8(trim(&line[..eq])) else {
            continue;
        };
        let Some(key) = key
            .strip_prefix("XDG_")
            .and_then(|k| k.strip_suffix("_DIR"))
        else {
            continue;
        };

        let Some(which) = UserDir::ALL.into_iter().find(|d| key_for(*d) == Some(key)) else {
            continue;
        };

        // Values are shell-quoted, e.g. `XDG_DESKTOP_DIR="$HOME/Desktop"`.
        // `xdg_user_dir_lookup` requires the opening quote and skips the entry
        // without one, so an unquoted value must not be honoured here either:
        // doing so would resolve a directory that every GTK/Qt application on
        // the same machine ignores.
        let Some(raw) = trim(&line[eq + 1..]).strip_prefix(b"\"".as_slice()) else {
            continue;
        };

        // The `$HOME/` test runs on the raw bytes, before escapes are resolved,
        // matching the reference implementation: `\$HOME/x` is a literal path,
        // not a home-relative one.
        let (relative, rest) = match raw.strip_prefix(b"$HOME/".as_slice()) {
            Some(rest) => (true, rest),
            None if raw.first() == Some(&b'/') => (false, raw),
            // Neither home-relative nor absolute; the spec permits nothing else.
            None => continue,
        };

        // Everything up to the first unescaped quote. Trailing content after it
        // (typically a hand-written comment) is not part of the value.
        let Some(value) = unquote(rest) else {
            continue;
        };

        let path = if relative {
            home.join(path_from_bytes(&value))
        } else {
            path_from_bytes(&value)
        };

        // A directory pointed at the home directory is disabled. The
        // xdg-user-dirs README is explicit: "To disable a directory, point it
        // to the homedir."
        if path != home {
            out[which.index()] = Some(path);
        }
    }

    out
}

/// Read a shell-quoted value up to the first unescaped `"`, resolving
/// backslash escapes.
///
/// Returns `None` if the quote is never closed, which the reference
/// implementation also treats as a malformed entry to be skipped.
fn unquote(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter().copied();

    while let Some(byte) = iter.next() {
        match byte {
            b'"' => return Some(out),
            // A backslash escapes the next byte, whatever it is.
            b'\\' => out.push(iter.next()?),
            _ => out.push(byte),
        }
    }

    None
}

fn font_dir(home: &Path) -> PathBuf {
    data_home(home).join("fonts")
}

/// Read and parse a `user-dirs.dirs` file.
///
/// A missing or unreadable file is not an error: it just means no directory is
/// configured, which is the normal state on a machine where
/// `xdg-user-dirs-update` has never run.
fn read_user_dirs_file(file: &Path, home: &Path) -> [Option<PathBuf>; crate::COUNT] {
    match std::fs::read(file) {
        Ok(contents) => parse(&contents, home),
        Err(_) => Default::default(),
    }
}

fn read_user_dirs(home: &Path) -> [Option<PathBuf>; crate::COUNT] {
    read_user_dirs_file(&config_home(home).join("user-dirs.dirs"), home)
}

pub(crate) fn resolve(which: UserDir) -> Option<PathBuf> {
    let home = home()?;
    if which == UserDir::Font {
        return Some(font_dir(&home));
    }
    read_user_dirs(&home)[which.index()].clone()
}

pub(crate) fn resolve_all() -> Option<UserDirs> {
    let home = home()?;
    let mut dirs = read_user_dirs(&home);
    dirs[UserDir::Font.index()] = Some(font_dir(&home));
    Some(UserDirs { dirs })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/alice")
    }

    fn get(contents: &str, which: UserDir) -> Option<PathBuf> {
        get_bytes(contents.as_bytes(), which)
    }

    fn get_bytes(contents: &[u8], which: UserDir) -> Option<PathBuf> {
        parse(contents, &home())[which.index()].clone()
    }

    #[test]
    fn parses_a_typical_file() {
        let contents = concat!(
            "# This file is written by xdg-user-dirs-update\n",
            "XDG_DESKTOP_DIR=\"$HOME/Desktop\"\n",
            "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n",
            "XDG_MUSIC_DIR=\"$HOME/Music\"\n",
            "XDG_PUBLICSHARE_DIR=\"$HOME/Public\"\n",
        );
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
        assert_eq!(
            get(contents, UserDir::Audio),
            Some(PathBuf::from("/home/alice/Music"))
        );
        assert_eq!(
            get(contents, UserDir::Public),
            Some(PathBuf::from("/home/alice/Public"))
        );
        // Absent key stays absent.
        assert_eq!(get(contents, UserDir::Video), None);
    }

    #[test]
    fn accepts_absolute_paths_outside_home() {
        let contents = "XDG_DOWNLOAD_DIR=\"/mnt/bulk/dl\"\n";
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/mnt/bulk/dl"))
        );
    }

    #[test]
    fn home_valued_entry_means_disabled() {
        // xdg-user-dirs writes this to mean "no such directory".
        assert_eq!(get("XDG_DOWNLOAD_DIR=\"$HOME\"\n", UserDir::Download), None);
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOME/\"\n", UserDir::Download),
            None
        );
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"/home/alice\"\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn rejects_relative_paths() {
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"Downloads\"\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn ignores_comments_junk_and_unknown_keys() {
        let contents = concat!(
            "# XDG_DOWNLOAD_DIR=\"$HOME/Commented\"\n",
            "\n",
            "not a key-value line\n",
            "XDG_UNKNOWN_DIR=\"$HOME/Nope\"\n",
            "SOMETHING_ELSE=\"$HOME/Nope\"\n",
            "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n",
        );
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
    }

    #[test]
    fn last_entry_wins() {
        let contents = concat!(
            "XDG_DOWNLOAD_DIR=\"$HOME/First\"\n",
            "XDG_DOWNLOAD_DIR=\"$HOME/Second\"\n",
        );
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/home/alice/Second"))
        );
    }

    #[test]
    fn tolerates_stray_whitespace_around_key_and_value() {
        let contents = "  XDG_DOWNLOAD_DIR = \"$HOME/Downloads\"  \n";
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn non_utf8_paths_survive_and_do_not_poison_the_file() {
        use std::os::unix::ffi::OsStrExt;

        // 0xFF is not valid UTF-8, but it is a perfectly legal byte in a Unix
        // path. Reading the file as a `String` would fail outright and discard
        // every entry, not just this one.
        let mut contents = Vec::new();
        contents.extend_from_slice(b"XDG_DOWNLOAD_DIR=\"$HOME/D\xFFwn\"\n");
        contents.extend_from_slice(b"XDG_MUSIC_DIR=\"$HOME/Music\"\n");

        let download = get_bytes(&contents, UserDir::Download).expect("download dir");
        assert_eq!(
            download.as_os_str().as_bytes(),
            b"/home/alice/D\xFFwn".as_slice()
        );

        // The neighbouring well-formed entry is unaffected.
        assert_eq!(
            get_bytes(&contents, UserDir::Audio),
            Some(PathBuf::from("/home/alice/Music"))
        );
    }

    #[test]
    fn trailing_content_after_the_closing_quote_is_ignored() {
        // A hand-edited file may carry a trailing comment. Treating it as part
        // of the value would silently disable the directory.
        assert_eq!(
            get(
                "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\" # my note\n",
                UserDir::Download
            ),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
    }

    #[test]
    fn value_ends_at_the_first_unescaped_quote() {
        // Scanning to the *last* quote instead of the first would yield
        // `Downloads" # "note`.
        assert_eq!(
            get(
                "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\" # \"note\"\n",
                UserDir::Download
            ),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
    }

    #[test]
    fn backslash_escapes_are_resolved() {
        // `xdg_user_dir_lookup` unescapes inside the quoted value, so a quote
        // or space in a directory name survives.
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOME/My\\\"Dir\"\n", UserDir::Download),
            Some(PathBuf::from("/home/alice/My\"Dir"))
        );
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOME/a\\ b\"\n", UserDir::Download),
            Some(PathBuf::from("/home/alice/a b"))
        );
    }

    #[test]
    fn escaped_home_is_not_expanded() {
        // The reference tests the `$HOME/` prefix before unescaping, so
        // `\$HOME/x` is neither home-relative nor absolute, and is skipped.
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"\\$HOME/x\"\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn unquoted_values_are_rejected() {
        // `xdg_user_dir_lookup` requires the opening quote. Honouring an
        // unquoted value would resolve a directory that GTK and Qt ignore.
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=$HOME/Downloads\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn projects_dir_is_parsed() {
        // `PROJECTS=Projects` ships in the xdg-user-dirs defaults.
        assert_eq!(
            get("XDG_PROJECTS_DIR=\"$HOME/Projects\"\n", UserDir::Project),
            Some(PathBuf::from("/home/alice/Projects"))
        );
    }

    #[test]
    fn unterminated_quote_is_rejected() {
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOME/Downloads\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn handles_crlf_line_endings() {
        let contents = "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\r\nXDG_MUSIC_DIR=\"$HOME/Music\"\r\n";
        assert_eq!(
            get(contents, UserDir::Download),
            Some(PathBuf::from("/home/alice/Downloads"))
        );
        assert_eq!(
            get(contents, UserDir::Audio),
            Some(PathBuf::from("/home/alice/Music"))
        );
    }

    #[test]
    fn value_may_contain_an_equals_sign() {
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOME/a=b\"\n", UserDir::Download),
            Some(PathBuf::from("/home/alice/a=b"))
        );
    }

    #[test]
    fn home_prefix_must_be_a_path_component() {
        // `$HOMEwork` is not `$HOME` followed by a path.
        assert_eq!(
            get("XDG_DOWNLOAD_DIR=\"$HOMEwork\"\n", UserDir::Download),
            None
        );
    }

    #[test]
    fn base_dir_ignores_relative_and_empty_values() {
        use std::ffi::OsString;

        // An absolute value wins.
        assert_eq!(
            base_dir(Some(OsString::from("/xdg/config")), &home(), ".config"),
            PathBuf::from("/xdg/config")
        );
        // The spec requires a relative value to be treated as invalid.
        assert_eq!(
            base_dir(Some(OsString::from("relative/config")), &home(), ".config"),
            PathBuf::from("/home/alice/.config")
        );
        // As is an empty one.
        assert_eq!(
            base_dir(Some(OsString::new()), &home(), ".config"),
            PathBuf::from("/home/alice/.config")
        );
        // Unset falls back too.
        assert_eq!(
            base_dir(None, &home(), ".local/share"),
            PathBuf::from("/home/alice/.local/share")
        );
    }

    /// Create an empty directory under the system temp dir, unique to this
    /// process and call, so the I/O tests do not collide when run in parallel.
    fn scratch_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "userdirs-test-{}-{}-{}",
            std::process::id(),
            tag,
            unique
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn reads_a_real_file_from_disk() {
        let dir = scratch_dir("read");
        let file = dir.join("user-dirs.dirs");
        std::fs::write(&file, "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n").expect("write");

        let dirs = read_user_dirs_file(&file, &home());
        assert_eq!(
            dirs[UserDir::Download.index()],
            Some(PathBuf::from("/home/alice/Downloads"))
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_yields_no_directories_rather_than_failing() {
        let dir = scratch_dir("missing");
        let dirs = read_user_dirs_file(&dir.join("does-not-exist"), &home());
        assert!(dirs.iter().all(Option::is_none));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unreadable_path_yields_no_directories() {
        // Pointing at a directory rather than a file makes `read` fail; the
        // caller must still get an answer instead of a panic.
        let dir = scratch_dir("unreadable");
        let dirs = read_user_dirs_file(&dir, &home());
        assert!(dirs.iter().all(Option::is_none));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parser_never_panics_on_arbitrary_bytes() {
        // The parser slices on byte offsets it computes itself; walk a spread of
        // adversarial inputs to confirm none of that indexing can go out of
        // bounds. Values are irrelevant here, only that it returns.
        let cases: [&[u8]; 14] = [
            b"",
            b"=",
            b"\n\n\n",
            b"XDG_",
            b"XDG_DOWNLOAD_DIR",
            b"XDG_DOWNLOAD_DIR=",
            b"XDG_DOWNLOAD_DIR=\"",
            b"XDG_DOWNLOAD_DIR=\"\\",
            b"XDG_DOWNLOAD_DIR=\"$HOME/",
            b"XDG_DOWNLOAD_DIR=\"$HOME",
            b"=XDG_DOWNLOAD_DIR",
            b"\xFF\xFE=\xFF",
            b"XDG_\xFF_DIR=\"/x\"",
            b"#XDG_DOWNLOAD_DIR=\"/x\"",
        ];
        for case in cases {
            let _ = parse(case, &home());
        }
    }

    #[test]
    fn font_dir_defaults_under_data_home() {
        // Only exercises the default branch; XDG_DATA_HOME is process-global
        // and setting it here would race other tests.
        let dir = super::font_dir(&home());
        assert!(dir.ends_with("fonts"), "{dir:?}");
    }
}
