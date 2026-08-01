//! Windows.
//!
//! Resolves paths through the Known Folder API rather than assuming a layout
//! under the profile directory, because these folders are relocatable.

use crate::{UserDir, UserDirs};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Public, FOLDERID_Templates, FOLDERID_Videos, SHGetKnownFolderPath,
    KF_FLAG_DONT_VERIFY,
};

fn folder_id(which: UserDir) -> Option<GUID> {
    Some(match which {
        UserDir::Audio => FOLDERID_Music,
        UserDir::Desktop => FOLDERID_Desktop,
        UserDir::Document => FOLDERID_Documents,
        UserDir::Download => FOLDERID_Downloads,
        UserDir::Picture => FOLDERID_Pictures,
        UserDir::Public => FOLDERID_Public,
        UserDir::Template => FOLDERID_Templates,
        UserDir::Video => FOLDERID_Videos,
        // The Known Folder API exposes no per-user font folder, and Windows has
        // no projects folder at all.
        UserDir::Font | UserDir::Project => return None,
    })
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path_ptr: *mut u16 = std::ptr::null_mut();

    // SAFETY: `id` is a valid GUID reference and `path_ptr` is a valid out
    // pointer. On success the callee allocates a NUL-terminated wide string
    // that we must release with `CoTaskMemFree`.
    let result = unsafe {
        SHGetKnownFolderPath(
            id,
            KF_FLAG_DONT_VERIFY as u32,
            std::ptr::null_mut(),
            &mut path_ptr,
        )
    };

    // A non-zero HRESULT can still hand back an allocation, so free
    // unconditionally once we know the pointer is non-null.
    if path_ptr.is_null() {
        return None;
    }

    // SAFETY: `path_ptr` is non-null and, on success, points at a
    // NUL-terminated wide string allocated by the shell, so scanning for the
    // terminator stays in bounds. Done by hand rather than with `lstrlenW` to
    // avoid pulling in the `Win32_Globalization` feature for one call.
    let path = (result >= 0).then(|| unsafe {
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(path_ptr, len))
    });

    // SAFETY: `path_ptr` was allocated by `SHGetKnownFolderPath` and has not
    // been freed. Any borrow of it ended above.
    unsafe { CoTaskMemFree(path_ptr as *const _) };

    path.map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

pub(crate) fn resolve(which: UserDir) -> Option<PathBuf> {
    known_folder(&folder_id(which)?)
}

pub(crate) fn resolve_all() -> Option<UserDirs> {
    let mut dirs: [Option<PathBuf>; crate::COUNT] = Default::default();
    for which in UserDir::ALL {
        dirs[which.index()] = resolve(which);
    }
    // Mirrors the other platforms: `None` means "no home at all", and on
    // Windows the shell always has an answer for at least one of these.
    dirs.iter()
        .any(Option::is_some)
        .then_some(UserDirs { dirs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_dir_is_absent() {
        // `GUID` implements neither `PartialEq` nor `Debug`.
        assert!(folder_id(UserDir::Font).is_none());
        assert_eq!(resolve(UserDir::Font), None);
    }

    #[test]
    fn known_folders_resolve_to_absolute_paths() {
        for which in UserDir::ALL {
            if let Some(path) = resolve(which) {
                assert!(path.is_absolute(), "{which:?} -> {path:?}");
            }
        }
    }

    #[test]
    fn download_dir_resolves() {
        // FOLDERID_Downloads exists on every supported Windows version.
        assert!(resolve(UserDir::Download).is_some());
    }

    #[test]
    fn resolve_all_agrees_with_resolve() {
        let all = resolve_all().expect("shell should resolve at least one known folder");
        for which in UserDir::ALL {
            assert_eq!(
                all.get(which).map(PathBuf::from),
                resolve(which),
                "{which:?}"
            );
        }
    }
}
