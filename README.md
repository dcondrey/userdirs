<!-- repo-header:start -->
<img src="https://raw.githubusercontent.com/dcondrey/userdirs/main/assets/logo.png" alt="userdirs logo" width="120" align="left">

<h1>userdirs</h1>

<p><strong>Cross-platform paths to the user&#x27;s media and personal directories. No copyleft dependencies.</strong></p>

<br clear="left">

[![CI](https://img.shields.io/github/actions/workflow/status/dcondrey/userdirs/ci.yml?style=flat-square&labelColor=20232a&branch=main&label=CI)](https://github.com/dcondrey/userdirs/actions/workflows/ci.yml) [![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14408/badge)](https://www.bestpractices.dev/projects/14408) [![License](https://img.shields.io/github/license/dcondrey/userdirs?style=flat-square&labelColor=20232a&color=007ec6&label=license)](https://github.com/dcondrey/userdirs/blob/main/LICENSE-APACHE) [![GitHub Sponsors](https://img.shields.io/badge/GitHub%20Sponsors-Sponsor-EA4AAA?style=flat-square&labelColor=20232a)](https://github.com/sponsors/dcondrey) [![crates.io](https://img.shields.io/crates/v/userdirs.svg?style=flat-square&labelColor=20232a&color=007ec6)](https://crates.io/crates/userdirs) [![docs.rs](https://img.shields.io/docsrs/userdirs?style=flat-square&labelColor=20232a&color=007ec6)](https://docs.rs/userdirs)
<!-- repo-header:end -->

```rust
if let Some(dir) = userdirs::download_dir() {
    println!("downloads live at {}", dir.display());
}
```

No copyleft anywhere in the dependency graph, and nothing at all to compile
outside Windows.

## Why this exists

`etcetera` covers the directories an *application* writes to — config, data,
cache, state, runtime — and deliberately stops there. The directories a person
keeps files in are a separate set, and the only cross-platform way to get them
was `dirs`, which reaches `dirs-sys` → `option-ext` and so drags an **MPL-2.0**
dependency into an otherwise MIT/Apache tree.

That blocks anyone whose license policy rejects copyleft even transitively, and
it is not going to change: five pull requests proposing the dependency's removal
have been declined, the most recent in January 2026. Real projects have left
over it — [zcash/librustzcash](https://github.com/zcash/librustzcash/pull/864)
dropped `directories` for exactly this reason.

The alternatives are all partial. `known-folders` is Windows-only. `xdg-user` is
Linux-only. `etcetera` covers none of this set by design.

So this is the missing piece, not a `dirs` replacement. Pair it with `etcetera`
and you have the full picture with a clean license graph.

## Platform behaviour

| function | Linux / BSD | macOS | Windows |
|---|---|---|---|
| `audio_dir` | `XDG_MUSIC_DIR` | `$HOME/Music` | `FOLDERID_Music` |
| `desktop_dir` | `XDG_DESKTOP_DIR` | `$HOME/Desktop` | `FOLDERID_Desktop` |
| `document_dir` | `XDG_DOCUMENTS_DIR` | `$HOME/Documents` | `FOLDERID_Documents` |
| `download_dir` | `XDG_DOWNLOAD_DIR` | `$HOME/Downloads` | `FOLDERID_Downloads` |
| `font_dir` | `$XDG_DATA_HOME/fonts` | `$HOME/Library/Fonts` | `%LOCALAPPDATA%\Microsoft\Windows\Fonts` |
| `picture_dir` | `XDG_PICTURES_DIR` | `$HOME/Pictures` | `FOLDERID_Pictures` |
| `project_dir` | `XDG_PROJECTS_DIR` | — | — |
| `public_dir` | `XDG_PUBLICSHARE_DIR` | `$HOME/Public` | `FOLDERID_Public` |
| `template_dir` | `XDG_TEMPLATES_DIR` | — | `FOLDERID_Templates` |
| `video_dir` | `XDG_VIDEOS_DIR` | `$HOME/Movies` | `FOLDERID_Videos` |

Every function returns `None` where the platform has no such concept, or where
the home directory cannot be determined. **Returned paths are not guaranteed to
exist on disk** — they are where the directory belongs, not proof it is there.

Targets that are neither Unix nor Windows, `wasm32-unknown-unknown` in
particular, compile and return `None` throughout. Depending on this crate will
not break a build matrix that includes one.

## Differences from `dirs`

Names and return types match, so migrating those functions is mechanical. The
differences are deliberate:

**`project_dir()` is new.** `PROJECTS=Projects` ships in the `xdg-user-dirs`
defaults, so `XDG_PROJECTS_DIR` appears in real `user-dirs.dirs` files. `dirs`
never exposed it.

**`font_dir()` works on Windows,** where `dirs` returns `None`. Windows 10 1803
added per-user font installation at `%LOCALAPPDATA%\Microsoft\Windows\Fonts`,
which is where a non-elevated install writes. Note this is *not* `FOLDERID_Fonts`
— Microsoft documents that one as FIXED at `%windir%\Fonts`, the machine-wide
store, which is not a user directory.

**A directory pointed at `$HOME` is disabled.** The `xdg-user-dirs` README is
explicit: "To disable a directory, point it to the homedir." This crate returns
`None`; `dirs` hands back the home directory. That applies only to a value
resolving to `$HOME` *itself* — `"$HOME/Downloads"` expands normally.

**Non-UTF-8 paths work.** Unix paths are arbitrary bytes. The parser reads
bytes, so one undecodable byte affects at most its own entry instead of
discarding the whole file.

**Missing folders still resolve on Windows.** Known-folder lookups pass
`KF_FLAG_DONT_VERIFY`, so a folder that does not exist yet still yields its
path. `dirs` verifies and returns `None`, which makes Windows disagree with
macOS for the same call.

The `user-dirs.dirs` parser follows `xdg_user_dir_lookup` from `xdg-user-dirs`:
the opening quote is required, the value ends at the first unescaped `"`, and
backslash escapes are resolved. What this crate resolves is what GTK and Qt
applications resolve.

## Caching (optional)

Resolving costs a `user-dirs.dirs` read on Linux and one shell call per
directory on Windows. If you consult these paths repeatedly:

```toml
userdirs = { version = "0.1", features = ["cache"] }
```

```rust
let dirs = userdirs::cache::user_dirs().unwrap();
println!("{}", dirs.download_dir().unwrap().display());

userdirs::cache::reload(); // after the directories may have moved
```

Measured on macOS: **8 ns cached against 470 ns uncached.** On Linux, where the
uncached path includes the file read, the gap is roughly three orders of
magnitude.

The feature only *adds* `userdirs::cache`; it never changes what the free
functions return. Cargo unifies features across a dependency graph, so if
enabling `cache` silently rewired `download_dir()`, an unrelated crate could
switch your lookups to cached values without your knowledge. Staleness is
something you ask for at the call site.

Snapshots do go stale — `xdg-user-dirs-update` can rewrite the file while you
run — so call `reload()` wherever the directories may have moved. GLib takes the
same approach with `g_reload_user_special_dirs_cache()`.

If you need several directories but not a process-wide cache, `UserDirs::new()`
resolves all of them from a single file read.

## Dependencies

| target | dependencies |
|---|---|
| Windows | `windows-sys` |
| everything else | none |

No `libc`. `std::env::home_dir()` was corrected and un-deprecated in Rust 1.85,
removing the `getpwuid_r` fallback that `dirs-sys` still carries `libc` for. It
prefers `$HOME` when set, so paths expand against the same home directory the
rest of the desktop resolves `user-dirs.dirs` against.

MSRV is 1.87, verified in CI against that exact toolchain.

## Testing

CI runs the suite on Linux, macOS and Windows, cross-checks
`x86_64-unknown-freebsd` and `wasm32-unknown-unknown`, and pins an MSRV job. One
job installs `xdg-user-dirs`, runs `xdg-user-dirs-update --force` and tests
against a genuinely generated file rather than a fixture.

The parser is exercised on every platform, not only XDG ones, so its tests run
wherever `cargo test` runs.

## License

MIT OR Apache-2.0, at your option, with no copyleft anywhere in the dependency
graph.
