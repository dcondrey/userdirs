# userdirs

Cross-platform paths to the user's media and personal directories: Downloads,
Documents, Pictures, Music, Videos, Desktop, Fonts, Templates, Public.

```rust
if let Some(dir) = userdirs::download_dir() {
    println!("downloads live at {}", dir.display());
}
```

## Why this exists

`etcetera` covers the directories an *application* writes to: config, data,
cache, state, runtime. It deliberately stops there. The directories a *person*
keeps files in are a separate set, and until now the only cross-platform way to
get them was `dirs`, which reaches `dirs-sys` → `option-ext` and so carries an
MPL-2.0 dependency into an otherwise MIT/Apache tree. That is a blocker at
organisations whose license policy rejects copyleft even transitively, and it is
[not going to change](https://codeberg.org/dirs/dirs-sys-rs/pulls?state=closed&q=option-ext)
— five PRs proposing its removal have been declined.

The alternatives are all partial: `known-folders` is Windows-only, `xdg-user` is
Linux-only.

This crate is the missing piece, not a `dirs` replacement. Pair it with
`etcetera` and you have the full set with no copyleft in the graph.

## Dependencies

| target | dependencies |
|---|---|
| Windows | `windows-sys` |
| everything else | none |

No `libc`. `std::env::home_dir()` was corrected and un-deprecated in Rust 1.85,
which removes the `getpwuid_r` fallback that `dirs-sys` still carries `libc`
for. MSRV is 1.87.

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
the home directory cannot be determined. Returned paths are not guaranteed to
exist on disk.

Targets that are neither Unix nor Windows — `wasm32-unknown-unknown` in
particular — compile and return `None` from every function. Depending on this
crate will not break a build matrix that includes one.

## Differences from `dirs`

Function names and return types match `dirs`, so migration is mechanical for
the nine it also provides. The differences, all deliberate:

- **`project_dir()` is new.** `PROJECTS=Projects` ships in the `xdg-user-dirs`
  defaults, so `XDG_PROJECTS_DIR` is written to real `user-dirs.dirs` files;
  `dirs` never exposed it.
- **`font_dir()` works on Windows.** `dirs` returns `None`. Windows 10 1803
  added per-user font installation at `%LOCALAPPDATA%\Microsoft\Windows\Fonts`,
  which is where a non-elevated install writes. Note this is not
  `FOLDERID_Fonts` — Microsoft documents that one as FIXED at `%windir%\Fonts`,
  the machine-wide store, which is not a user directory.
- **A directory pointed at `$HOME` is disabled.** The `xdg-user-dirs` README
  says so outright: "To disable a directory, point it to the homedir."
  `userdirs` returns `None`; `dirs` hands back the home directory. Note this is
  about a value that resolves to `$HOME` *itself* — `"$HOME/Downloads"` expands
  normally.
- **Non-UTF-8 paths work.** Unix paths are arbitrary bytes. The parser reads
  bytes, so one undecodable byte affects at most its own entry rather than
  discarding the whole file.
- **`UserDirs::new()`** parses `user-dirs.dirs` once. The free functions re-read
  it per call, as `dirs` does.

The `user-dirs.dirs` parser follows `xdg_user_dir_lookup` from `xdg-user-dirs`:
the opening quote is required, the value ends at the first unescaped `"`, and
backslash escapes are resolved. Entries this crate resolves are the entries GTK
and Qt applications resolve.

## License

MIT OR Apache-2.0, with no copyleft anywhere in the dependency graph.
