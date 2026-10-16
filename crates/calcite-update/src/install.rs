// SPDX-License-Identifier: MPL-2.0

//! Moves a staged tree into place, and finds the installed tree of the running app.
//!
//! | Platform        | Root                     | Swap                                         |
//! | --------------- | ------------------------ | -------------------------------------------- |
//! | macOS           | the `.app` bundle        | rename the bundle aside, rename the new one in |
//! | Linux, AppImage | the `.AppImage` file     | rename the new file over the old one         |
//! | Linux, folder   | the executable's folder  | whole-folder rename if the folder holds only files this app installed, else per file |
//! | Windows         | the executable's folder  | per file: the old file moves to `*.calcite-old`, which [`cleanup`] removes at the next launch |
//!
//! In a folder that is not a `.app` bundle, an install writes `.calcite-install.json`: the
//! app ID, the version, and the paths it owns. An install never deletes a file that the old
//! marker and the new tree do not list. A per-file install writes a journal first and undoes
//! itself on any failure; [`cleanup`] undoes one that a crash interrupted.
//!
//! A deb or pacman install belongs to the package manager; [`installed_root`] returns
//! `None` for it.

use std::{
	collections::BTreeSet,
	fs,
	io::Read,
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
	Entry, Error, Kind, Result, fsutil,
	journal::{self, JOURNAL, OLD_SUFFIX},
	manifest::validate_path,
	stage,
};

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

/// Names the owner and the files of an install in a folder that the app shares with other
/// files, such as the executable's folder on Windows and Linux.
pub(crate) const MARKER: &str = ".calcite-install.json";
/// The largest marker a client reads.
const MAX_MARKER: u64 = 16 << 20;
/// Suffix of the record that says an install made the `.calcite-old` sibling of a root.
const SWAP_RECORD_SUFFIX: &str = ".calcite-swap";

#[derive(Serialize, Deserialize)]
struct Marker {
	app_id: String,
	version: String,
	/// Every path the install wrote, relative to the root.
	owned: Vec<String>,
}

fn read_marker(root: &Path) -> Option<Marker> {
	let mut text = String::new();
	fsutil::open_regular(&root.join(MARKER))
		.ok()?
		.take(MAX_MARKER)
		.read_to_string(&mut text)
		.ok()?;
	let marker: Marker = serde_json::from_str(&text).ok()?;
	marker
		.owned
		.iter()
		.all(|path| validate_path(path).is_ok() && path != MARKER && path != JOURNAL)
		.then_some(marker)
}

/// A macOS `.app` bundle belongs to the app as a whole, so it needs no marker.
fn is_bundle(root: &Path) -> bool {
	cfg!(target_os = "macos") && root.extension().is_some_and(|extension| extension == "app")
}

/// Replaces the tree at `root` with the staged tree. On failure the old tree stays.
///
/// The staged tree is hashed again first. A `.app` bundle on macOS is swapped whole. In any
/// other folder, the whole folder is swapped only when the install marker names `product` and
/// the folder holds nothing but the marker and the files it lists. Otherwise the install
/// moves the listed files one by one, through a journal, and never touches an unlisted file.
pub fn install(staging: &Path, entries: &[Entry], root: &Path, product: &str, version: &str) -> Result<()> {
	stage::verify_tree(staging, entries)?;
	let root_is_file = fs::metadata(root)
		.map(|metadata| metadata.is_file())
		.unwrap_or(false);
	if root_is_file {
		return install_file(staging, entries, root);
	}
	if is_bundle(root) {
		return install_swap(staging, root);
	}
	if entries
		.iter()
		.any(|entry| entry.path == MARKER || entry.path == JOURNAL)
	{
		return Err(Error::Feed(format!(
			"the tree may not hold `{MARKER}` or `{JOURNAL}`"
		)));
	}
	journal::recover(root)?;
	let earlier = read_marker(root).filter(|marker| marker.app_id == product);
	write_marker(staging, entries, product, version)?;
	if !cfg!(windows)
		&& let Some(earlier) = &earlier
		&& only_owned(root, earlier)
	{
		return install_swap(staging, root);
	}

	let current: BTreeSet<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
	let retired: Vec<String> = earlier
		.iter()
		.flat_map(|marker| &marker.owned)
		.filter(|path| !current.contains(path.as_str()))
		.cloned()
		.collect();
	journal::install(staging, entries, &[MARKER], &retired, root)?;
	// Folders of the earlier install that are now empty; a folder with other files stays.
	for path in retired.iter().rev() {
		if let Ok(directory) = journal::safe_join(root, path) {
			let _ = fs::remove_dir(directory);
		}
	}
	let _ = fs::remove_dir_all(staging);
	Ok(())
}

fn write_marker(staging: &Path, entries: &[Entry], product: &str, version: &str) -> Result<()> {
	let marker = Marker {
		app_id: product.to_string(),
		version: version.to_string(),
		owned: entries.iter().map(|entry| entry.path.clone()).collect(),
	};
	let path = staging.join(MARKER);
	let text = serde_json::to_vec_pretty(&marker).map_err(|error| Error::Feed(error.to_string()))?;
	fs::write(&path, text).map_err(|error| Error::io(&path, error))
}

/// Whether `root` holds nothing besides the marker and what the marker lists.
fn only_owned(root: &Path, marker: &Marker) -> bool {
	let owned: BTreeSet<&str> = marker.owned.iter().map(String::as_str).collect();
	journal::contents(root).is_ok_and(|found| {
		found
			.iter()
			.all(|path| path == MARKER || owned.contains(path.as_str()))
	})
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
	let record = sibling(root, SWAP_RECORD_SUFFIX);
	if fs::symlink_metadata(&old).is_ok() {
		// Only a folder that an earlier swap recorded is mine to delete.
		if fs::symlink_metadata(&record).is_err() {
			return Err(Error::Integrity(format!(
				"{} is in the way and no install made it",
				old.display()
			)));
		}
		fs::remove_dir_all(&old).map_err(|error| Error::io(&old, error))?;
	}
	fsutil::write_atomic(&record, b"calcite-update swap").map_err(|error| Error::io(&record, error))?;
	if let Err(error) = fs::rename(root, &old) {
		let _ = fs::remove_file(&record);
		return Err(Error::io(root, error));
	}
	if let Err(error) = fs::rename(staging, root) {
		// Put the old tree back so the app still starts.
		fs::rename(&old, root).map_err(|restore| Error::io(root, restore))?;
		let _ = fs::remove_file(&record);
		return Err(Error::io(root, error));
	}
	match fs::remove_dir_all(&old) {
		Ok(()) => {
			let _ = fs::remove_file(&record);
		}
		Err(error) => eprintln!("calcite-update: could not remove {}: {error}", old.display()),
	}
	Ok(())
}

/// Settles what an earlier install left behind: restores the files of an interrupted
/// per-file install or removes the leftovers of a finished one (see the install journal),
/// and removes the staging and download folders. It walks no folder and removes only paths
/// that an install recorded. Call it early at launch; it never fails the app.
pub fn cleanup(root: &Path) {
	let remove = |path: &Path| {
		let result = match fs::symlink_metadata(path) {
			Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
			Ok(_) => fs::remove_file(path),
			Err(error) => Err(error),
		};
		if let Err(error) = result
			&& error.kind() != std::io::ErrorKind::NotFound
		{
			eprintln!("calcite-update: could not remove {}: {error}", path.display());
		}
	};
	if root.is_dir()
		&& let Err(error) = journal::recover(root)
	{
		eprintln!("calcite-update: {error}");
	}
	// A swap that stopped between its two renames leaves no root; the old tree comes back.
	// The sibling goes only if a swap recorded it.
	let old = sibling(root, OLD_SUFFIX);
	let record = sibling(root, SWAP_RECORD_SUFFIX);
	if fs::symlink_metadata(&old).is_err() {
		remove(&record);
	} else if fs::symlink_metadata(&record).is_err() {
		eprintln!(
			"calcite-update: leaving {}, which no install recorded",
			old.display()
		);
	} else if fs::symlink_metadata(root).is_err() {
		match fs::rename(&old, root) {
			Ok(()) => remove(&record),
			Err(error) => eprintln!("calcite-update: could not restore {}: {error}", root.display()),
		}
	} else {
		remove(&old);
		remove(&record);
	}
	remove(&staging_path(root));
	remove(&download_path(root));
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fsutil::{TempDir, hash_file};

	/// Writes `files` below `dir` (`None` makes a folder) and returns their entries.
	fn stage_tree(dir: &Path, files: &[(&str, Option<&str>)]) -> Vec<Entry> {
		let _ = fs::remove_dir_all(dir);
		fs::create_dir_all(dir).unwrap();
		files
			.iter()
			.map(|(path, content)| {
				let full = journal::join(dir, path);
				let kind = match content {
					None => {
						fs::create_dir(&full).unwrap();
						Kind::Directory
					}
					Some(content) => {
						fs::write(&full, content).unwrap();
						let (size, sha256) = hash_file(&full).unwrap();
						Kind::File {
							size,
							sha256,
							executable: false,
						}
					}
				};
				Entry {
					path: path.to_string(),
					kind,
				}
			})
			.collect()
	}

	fn install_tree(temp: &Path, root: &Path, product: &str, files: &[(&str, Option<&str>)]) -> Result<()> {
		let staging = temp.join("staging");
		let entries = stage_tree(&staging, files);
		install(&staging, &entries, root, product, "1.0.0")
	}

	/// Identifies the root folder: a whole-folder swap gives it a new one.
	#[cfg(unix)]
	fn folder_id(root: &Path) -> u64 {
		use std::os::unix::fs::MetadataExt;
		fs::metadata(root).unwrap().ino()
	}

	fn text(root: &Path, path: &str) -> String {
		fs::read_to_string(journal::join(root, path)).unwrap()
	}

	#[cfg(unix)]
	#[test]
	fn a_second_update_swaps_the_whole_folder_when_the_marker_matches() {
		let temp = TempDir::new();
		let root = temp.0.join("root");
		fs::create_dir(&root).unwrap();
		install_tree(&temp.0, &root, "app", &[("d", None), ("d/a", Some("1"))]).unwrap();
		let first = folder_id(&root);

		install_tree(&temp.0, &root, "app", &[("d", None), ("d/a", Some("2"))]).unwrap();

		assert_ne!(folder_id(&root), first, "the folder was replaced");
		assert_eq!(text(&root, "d/a"), "2");
		assert!(root.join(MARKER).exists());
	}

	#[cfg(unix)]
	#[test]
	fn an_unowned_file_or_another_app_id_keeps_the_install_per_file() {
		for (unowned, second_product) in [(true, "app"), (false, "other")] {
			let temp = TempDir::new();
			let root = temp.0.join("root");
			fs::create_dir(&root).unwrap();
			install_tree(&temp.0, &root, "app", &[("a", Some("1"))]).unwrap();
			if unowned {
				fs::write(root.join("mine.txt"), "mine").unwrap();
			}
			let first = folder_id(&root);

			install_tree(&temp.0, &root, second_product, &[("b", Some("2"))]).unwrap();

			assert_eq!(folder_id(&root), first, "the folder stayed");
			assert_eq!(text(&root, "b"), "2");
			if unowned {
				assert_eq!(text(&root, "mine.txt"), "mine");
			}
		}
	}

	#[test]
	fn a_per_file_install_retires_dropped_files_and_their_empty_folders() {
		let temp = TempDir::new();
		let root = temp.0.join("root");
		fs::create_dir(&root).unwrap();
		let first = [("d", None), ("d/gone", Some("x")), ("keep", Some("k"))];
		install_tree(&temp.0, &root, "app", &first).unwrap();
		fs::write(root.join("mine.txt"), "mine").unwrap();

		install_tree(&temp.0, &root, "app", &[("keep", Some("k2"))]).unwrap();

		assert!(
			!root.join("d").exists(),
			"the dropped file and its folder are gone"
		);
		assert_eq!(text(&root, "keep"), "k2");
		assert_eq!(text(&root, "mine.txt"), "mine");
		assert!(!root.join("keep.calcite-old").exists());
	}

	#[test]
	fn a_tampered_marker_is_not_trusted() {
		let temp = TempDir::new();
		let root = temp.0.join("root");
		fs::create_dir(&root).unwrap();
		let marker = |owned: &str| {
			let text = format!(r#"{{"app_id":"app","version":"1","owned":{owned}}}"#);
			fs::write(root.join(MARKER), text).unwrap();
			read_marker(&root)
		};
		assert!(marker(r#"["a","d/b"]"#).is_some());
		for unsafe_owned in [
			r#"["../escape"]"#,
			r#"["/etc/passwd"]"#,
			r#"["a//b"]"#,
			r#"[".calcite-install.json"]"#,
		] {
			assert!(marker(unsafe_owned).is_none(), "{unsafe_owned}");
		}
		fs::write(root.join(MARKER), "not json").unwrap();
		assert!(read_marker(&root).is_none());
	}

	#[test]
	fn a_tree_may_not_hold_the_marker_or_journal_names() {
		for name in [MARKER, JOURNAL] {
			let temp = TempDir::new();
			let root = temp.0.join("root");
			fs::create_dir(&root).unwrap();
			let result = install_tree(&temp.0, &root, "app", &[(name, Some("x"))]);
			assert!(matches!(result, Err(Error::Feed(_))), "{name}");
		}
	}
}
