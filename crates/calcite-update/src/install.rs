// SPDX-License-Identifier: MPL-2.0

//! Moves a staged tree into place, and finds the installed tree of the running app.
//!
//! | Platform        | Root                     | Swap                                         |
//! | --------------- | ------------------------ | -------------------------------------------- |
//! | macOS           | the `.app` bundle        | rename the bundle aside, rename the new one in |
//! | Linux, AppImage | the `.AppImage` file     | rename the new file over the old one         |
//! | Linux, folder   | the executable's folder  | like macOS                                   |
//! | Windows         | the executable's folder  | per file: the old file moves to `*.calcite-old`, which [`cleanup`] removes at the next launch |
//!
//! A deb or pacman install belongs to the package manager; [`installed_root`] returns
//! `None` for it.

use std::{
	fs,
	path::{Path, PathBuf},
};

use crate::{Entry, Error, Kind, Result};

/// Suffix of what an install leaves behind for [`cleanup`].
pub(crate) const OLD_SUFFIX: &str = ".calcite-old";

/// The installed tree of the running app, or `None` when something else updates it.
pub fn installed_root() -> Option<PathBuf> {
	if let Some(appimage) = std::env::var_os("APPIMAGE") {
		return Some(PathBuf::from(appimage));
	}
	let executable = std::env::current_exe().ok()?.canonicalize().ok()?;
	if cfg!(target_os = "macos") {
		// …/Name.app/Contents/MacOS/name
		let bundle = executable.parent()?.parent()?.parent()?;
		return (bundle.extension()? == "app").then(|| bundle.to_path_buf());
	}
	let folder = executable.parent()?;
	let managed = ["/usr", "/bin", "/opt/homebrew", "/nix"]
		.iter()
		.any(|prefix| folder.starts_with(prefix));
	(!managed).then(|| folder.to_path_buf())
}

/// Where the staged tree for `root` goes: next to it, on the same file system, so the swap
/// is a rename.
pub fn staging_path(root: &Path) -> PathBuf {
	sibling(root, ".calcite-staged")
}

/// Where downloads for `root` go.
pub fn download_path(root: &Path) -> PathBuf {
	sibling(root, ".calcite-download")
}

fn sibling(root: &Path, suffix: &str) -> PathBuf {
	let name = root
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	root.with_file_name(format!(".{name}{suffix}"))
}

/// Replaces the tree at `root` with the staged tree. On failure the old tree stays.
pub fn install(staging: &Path, entries: &[Entry], root: &Path) -> Result<()> {
	let root_is_file = fs::metadata(root)
		.map(|metadata| metadata.is_file())
		.unwrap_or(false);
	if root_is_file {
		return install_file(staging, entries, root);
	}
	if cfg!(windows) {
		install_per_file(staging, entries, root)
	} else {
		install_swap(staging, root)
	}
}

/// A single-file app such as an AppImage: the tree is one file named like the root.
fn install_file(staging: &Path, entries: &[Entry], root: &Path) -> Result<()> {
	let [
		Entry {
			path,
			kind: Kind::File { .. },
		},
	] = entries
	else {
		return Err(Error::Feed("a single-file app needs a tree of one file".into()));
	};
	let staged = staging.join(path);
	fs::rename(&staged, root).map_err(|error| Error::io(root, error))?;
	let _ = fs::remove_dir_all(staging);
	Ok(())
}

fn install_swap(staging: &Path, root: &Path) -> Result<()> {
	let old = sibling(root, OLD_SUFFIX);
	if fs::symlink_metadata(&old).is_ok() {
		fs::remove_dir_all(&old).map_err(|error| Error::io(&old, error))?;
	}
	fs::rename(root, &old).map_err(|error| Error::io(root, error))?;
	if let Err(error) = fs::rename(staging, root) {
		// Put the old tree back so the app still starts.
		fs::rename(&old, root).map_err(|restore| Error::io(root, restore))?;
		return Err(Error::io(root, error));
	}
	if let Err(error) = fs::remove_dir_all(&old) {
		eprintln!("calcite-update: could not remove {}: {error}", old.display());
	}
	Ok(())
}

/// Windows cannot rename a folder with a running executable, but it can rename the
/// executable itself. Each changed file moves aside before the new one moves in.
fn install_per_file(staging: &Path, entries: &[Entry], root: &Path) -> Result<()> {
	let join = |base: &Path, path: &str| {
		path.split('/')
			.fold(base.to_path_buf(), |path, part| path.join(part))
	};
	for entry in entries {
		let target = join(root, &entry.path);
		match entry.kind {
			Kind::Directory => fs::create_dir_all(&target).map_err(|error| Error::io(&target, error))?,
			Kind::Symlink { .. } => return Err(Error::Feed("symlinks are not supported on Windows".into())),
			Kind::File { .. } => {
				if fs::symlink_metadata(&target).is_ok() {
					let aside = target.with_file_name(format!(
						"{}{OLD_SUFFIX}",
						target.file_name().unwrap_or_default().to_string_lossy()
					));
					let _ = fs::remove_file(&aside);
					fs::rename(&target, &aside).map_err(|error| Error::io(&target, error))?;
				}
				let staged = join(staging, &entry.path);
				fs::rename(&staged, &target).map_err(|error| Error::io(&target, error))?;
			}
		}
	}
	let _ = fs::remove_dir_all(staging);
	Ok(())
}

/// Removes what an earlier install left behind: `*.calcite-old` files and folders, and
/// stale staging and download folders. Call it early at launch; it never fails the app.
pub fn cleanup(root: &Path) {
	let remove = |path: &Path| {
		let result = if path.is_dir() {
			fs::remove_dir_all(path)
		} else {
			fs::remove_file(path)
		};
		if let Err(error) = result
			&& error.kind() != std::io::ErrorKind::NotFound
		{
			eprintln!("calcite-update: could not remove {}: {error}", path.display());
		}
	};
	remove(&sibling(root, OLD_SUFFIX));
	remove(&staging_path(root));
	remove(&download_path(root));
	if root.is_dir() {
		let mut stack = vec![root.to_path_buf()];
		while let Some(directory) = stack.pop() {
			let Ok(read) = fs::read_dir(&directory) else {
				continue;
			};
			for item in read.flatten() {
				let path = item.path();
				let is_old = path
					.file_name()
					.is_some_and(|name| name.to_string_lossy().ends_with(OLD_SUFFIX));
				match item.file_type() {
					_ if is_old => remove(&path),
					Ok(kind) if kind.is_dir() => stack.push(path),
					_ => {}
				}
			}
		}
	}
}
