// SPDX-License-Identifier: MPL-2.0

//! A per-file install that can be undone. The plan is written to a journal before the first
//! file moves. Any failure restores every moved file, and so does a journal that a crash left
//! behind. Only paths in the plan are ever touched.

use std::{
	collections::BTreeSet,
	fs,
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Entry, Error, Kind, Result, fsutil, manifest::validate_path};

pub(crate) const JOURNAL: &str = ".calcite-install-journal.json";
/// Suffix of a file that an install moved aside.
pub(crate) const OLD_SUFFIX: &str = ".calcite-old";

#[derive(Serialize, Deserialize)]
struct Journal {
	/// Set once every file is in place. A committed journal only waits for its leftovers to go.
	committed: bool,
	/// Folders the install creates.
	directories: Vec<String>,
	operations: Vec<Operation>,
}

#[derive(Serialize, Deserialize)]
struct Operation {
	path: String,
	/// Where the existing file moves before the new one arrives, or before it is retired.
	aside: Option<String>,
	/// Whether the staged file takes the place of `path`.
	install: bool,
	/// What the installed file looks like (`sha256:<hex>` or `link:<target>`). A rollback
	/// removes a file without an aside only if it still matches.
	#[serde(default)]
	fingerprint: String,
}

pub(crate) fn join(root: &Path, path: &str) -> PathBuf {
	path.split('/')
		.fold(root.to_path_buf(), |path, part| path.join(part))
}

pub(crate) fn aside_name(path: &str) -> String {
	format!("{path}{OLD_SUFFIX}")
}

/// Joins a relative path to `root` after checking it: the path is lexically safe, and no
/// folder on the way is a link or a file. A path from a marker or a journal goes through this
/// before anything acts on it, so a planted link cannot lead out of `root`.
pub(crate) fn safe_join(root: &Path, path: &str) -> Result<PathBuf> {
	validate_path(path)?;
	let parts: Vec<&str> = path.split('/').collect();
	let mut full = root.to_path_buf();
	for (index, part) in parts.iter().enumerate() {
		full.push(part);
		if index + 1 < parts.len() {
			match fs::symlink_metadata(&full) {
				Ok(metadata) if metadata.is_dir() => {}
				Ok(_) => {
					return Err(Error::Integrity(format!(
						"`{path}` passes through a link or a file"
					)));
				}
				// Nothing exists from here on.
				Err(_) => break,
			}
		}
	}
	Ok(join(root, path))
}

fn fingerprint(kind: &Kind) -> String {
	match kind {
		Kind::File { sha256, .. } => format!("sha256:{sha256}"),
		Kind::Symlink { target } => format!("link:{target}"),
		Kind::Directory => String::new(),
	}
}

fn matches_fingerprint(path: &Path, fingerprint: &str) -> bool {
	if let Some(target) = fingerprint.strip_prefix("link:") {
		fs::read_link(path).is_ok_and(|actual| actual.to_string_lossy() == target)
	} else if let Some(sha256) = fingerprint.strip_prefix("sha256:") {
		fsutil::hash_file(path).is_ok_and(|(_, actual)| actual == sha256)
	} else {
		false
	}
}

fn exists(path: &Path) -> bool {
	fs::symlink_metadata(path).is_ok()
}

/// Installs `entries` from `staging` into `root`, file by file. `extra` names staged files
/// outside `entries` (the install marker). `retire` lists files of an earlier install that the
/// new tree no longer holds; they move aside too. Nothing else in `root` is touched.
pub(crate) fn install(
	staging: &Path,
	entries: &[Entry],
	extra: &[&str],
	retire: &[String],
	root: &Path,
) -> Result<()> {
	let mut journal = Journal {
		committed: false,
		directories: Vec::new(),
		operations: Vec::new(),
	};
	for entry in entries {
		let target = safe_join(root, &entry.path)?;
		match &entry.kind {
			Kind::Directory => match fs::symlink_metadata(&target) {
				Ok(metadata) if metadata.is_dir() => {}
				Ok(_) => {
					return Err(Error::Integrity(format!(
						"`{}` is not a folder in the installed copy",
						entry.path
					)));
				}
				Err(_) => journal.directories.push(entry.path.clone()),
			},
			Kind::Symlink { .. } if cfg!(windows) => {
				return Err(Error::Feed("symlinks are not supported on Windows".into()));
			}
			Kind::File { .. } | Kind::Symlink { .. } => {
				journal
					.operations
					.push(replace(&entry.path, &target, fingerprint(&entry.kind))?)
			}
		}
	}
	for path in extra {
		let staged = staging.join(path);
		let (_, sha256) = fsutil::hash_file(&staged).map_err(|error| Error::io(&staged, error))?;
		journal.operations.push(replace(
			path,
			&safe_join(root, path)?,
			format!("sha256:{sha256}"),
		)?);
	}
	for path in retire {
		let Ok(target) = safe_join(root, path) else {
			eprintln!("calcite-update: skipping unsafe retired path `{path}`");
			continue;
		};
		if fs::symlink_metadata(target).is_ok_and(|metadata| !metadata.is_dir()) {
			journal.operations.push(Operation {
				path: path.clone(),
				aside: Some(aside_name(path)),
				install: false,
				fingerprint: String::new(),
			});
		}
	}
	for operation in &journal.operations {
		if let Some(aside) = &operation.aside
			&& exists(&join(root, aside))
		{
			return Err(Error::Integrity(format!(
				"`{aside}` is in the way of the install and is not mine to delete"
			)));
		}
	}

	write(root, &journal)?;
	match apply(staging, root, &journal) {
		Ok(()) => {
			journal.committed = true;
			if let Err(error) = write(root, &journal) {
				rollback(root, &journal);
				return Err(error);
			}
		}
		Err(error) => {
			rollback(root, &journal);
			return Err(error);
		}
	}
	finalize(root, &journal);
	Ok(())
}

fn replace(path: &str, target: &Path, fingerprint: String) -> Result<Operation> {
	let existing = fs::symlink_metadata(target);
	if existing.as_ref().is_ok_and(|metadata| metadata.is_dir()) {
		return Err(Error::Integrity(format!(
			"`{path}` is a folder in the installed copy, not a file"
		)));
	}
	Ok(Operation {
		path: path.to_string(),
		aside: existing.is_ok().then(|| aside_name(path)),
		install: true,
		fingerprint,
	})
}

fn apply(staging: &Path, root: &Path, journal: &Journal) -> Result<()> {
	for directory in &journal.directories {
		let path = join(root, directory);
		fs::create_dir(&path).map_err(|error| Error::io(&path, error))?;
	}
	for operation in &journal.operations {
		let target = join(root, &operation.path);
		if let Some(aside) = &operation.aside {
			fs::rename(&target, join(root, aside)).map_err(|error| Error::io(&target, error))?;
		}
		if operation.install {
			fs::rename(join(staging, &operation.path), &target).map_err(|error| Error::io(&target, error))?;
		}
	}
	Ok(())
}

fn write(root: &Path, journal: &Journal) -> Result<()> {
	let path = root.join(JOURNAL);
	let text = serde_json::to_vec(journal).map_err(|error| Error::Feed(error.to_string()))?;
	fsutil::write_atomic(&path, &text).map_err(|error| Error::io(&path, error))
}

/// Puts every moved file back. Each step checks what is on disk, so it can run twice.
/// Returns whether the root is whole again and the journal is gone.
fn rollback(root: &Path, journal: &Journal) -> bool {
	let mut whole = true;
	for operation in journal.operations.iter().rev() {
		let target = join(root, &operation.path);
		match &operation.aside {
			Some(aside) => {
				let aside = join(root, aside);
				// Without the aside file, this step never ran and `target` is the original.
				if exists(&aside) {
					if exists(&target) && fs::remove_file(&target).is_err() {
						whole = false;
						continue;
					}
					if let Err(error) = fs::rename(&aside, &target) {
						eprintln!("calcite-update: could not restore {}: {error}", target.display());
						whole = false;
					}
				}
			}
			None => {
				// The plan found nothing here, so the new file is the only one to remove.
				if exists(&target) {
					if !matches_fingerprint(&target, &operation.fingerprint) {
						eprintln!("calcite-update: leaving {}, which is not mine", target.display());
					} else if let Err(error) = fs::remove_file(&target) {
						eprintln!("calcite-update: could not remove {}: {error}", target.display());
						whole = false;
					}
				}
			}
		}
	}
	for directory in journal.directories.iter().rev() {
		// Fails, and stays, when the folder holds something this install did not put there.
		let _ = fs::remove_dir(join(root, directory));
	}
	if whole {
		let _ = fs::remove_file(root.join(JOURNAL));
	}
	whole
}

/// Deletes the files that the install moved aside, then the journal. A file that cannot go
/// yet, such as a running executable on Windows, keeps the journal for the next launch.
fn finalize(root: &Path, journal: &Journal) {
	let mut done = true;
	for operation in &journal.operations {
		if let Some(aside) = &operation.aside {
			let path = join(root, aside);
			if let Err(error) = fs::remove_file(&path)
				&& error.kind() != std::io::ErrorKind::NotFound
			{
				eprintln!("calcite-update: could not remove {}: {error}", path.display());
				done = false;
			}
		}
	}
	if done {
		let _ = fs::remove_file(root.join(JOURNAL));
	}
}

/// Settles an install that did not finish: restores the old files, or removes the leftovers
/// of a finished one. Does nothing without a journal.
pub(crate) fn recover(root: &Path) -> Result<()> {
	let path = root.join(JOURNAL);
	let text = match fs::read_to_string(&path) {
		Ok(text) => text,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(Error::io(&path, error)),
	};
	let journal: Journal =
		serde_json::from_str(&text).map_err(|error| Error::Feed(format!("{}: {error}", path.display())))?;
	check(root, &journal)?;
	if journal.committed {
		finalize(root, &journal);
	} else if !rollback(root, &journal) {
		return Err(Error::Integrity(
			"the interrupted install could not be undone".into(),
		));
	}
	Ok(())
}

/// Rejects a journal that names a path it could not have written: an unsafe path, a path
/// behind a link, an aside file that is not `<path>.calcite-old`, or a new file without a
/// fingerprint.
fn check(root: &Path, journal: &Journal) -> Result<()> {
	let bad = |what: &str| Err(Error::Feed(format!("the install journal is unsafe: {what}")));
	for directory in &journal.directories {
		safe_join(root, directory)?;
	}
	for operation in &journal.operations {
		safe_join(root, &operation.path)?;
		match &operation.aside {
			Some(aside) if *aside == aside_name(&operation.path) => {
				safe_join(root, aside)?;
			}
			Some(aside) => {
				return bad(&format!(
					"`{aside}` is not the aside file of `{}`",
					operation.path
				));
			}
			None if operation.install && !operation.fingerprint.is_empty() => {}
			None => {
				return bad(&format!(
					"`{}` has neither an aside file nor a fingerprint",
					operation.path
				));
			}
		}
	}
	Ok(())
}

/// Every relative path under `root`, without following links.
pub(crate) fn contents(root: &Path) -> std::io::Result<BTreeSet<String>> {
	fn walk(directory: &Path, prefix: &str, out: &mut BTreeSet<String>) -> std::io::Result<()> {
		for item in fs::read_dir(directory)? {
			let item = item?;
			let name = item.file_name().to_string_lossy().into_owned();
			let path = if prefix.is_empty() {
				name
			} else {
				format!("{prefix}/{name}")
			};
			if item.file_type()?.is_dir() {
				walk(&item.path(), &path, out)?;
			}
			out.insert(path);
		}
		Ok(())
	}
	let mut out = BTreeSet::new();
	walk(root, "", &mut out)?;
	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fsutil::TempDir;

	/// An entry with the real hash of the staged file, if there is one.
	fn file(staging: &Path, path: &str) -> Entry {
		let sha256 = fsutil::hash_file(&staging.join(path)).map_or_else(|_| "0".repeat(64), |hash| hash.1);
		Entry {
			path: path.into(),
			kind: Kind::File {
				size: 0,
				sha256,
				executable: false,
			},
		}
	}

	fn text(root: &Path, path: &str) -> String {
		fs::read_to_string(root.join(path)).unwrap()
	}

	#[test]
	fn a_failure_halfway_restores_every_file() {
		let temp = TempDir::new();
		let (root, staging) = (temp.0.join("root"), temp.0.join("staging"));
		fs::create_dir_all(&root).unwrap();
		fs::create_dir_all(&staging).unwrap();
		fs::write(root.join("a"), "old a").unwrap();
		fs::write(root.join("b"), "old b").unwrap();
		fs::write(staging.join("a"), "new a").unwrap();
		fs::write(staging.join("c"), "new c").unwrap();
		// `b` is not staged, so the install fails after it moved `a` and `b` aside.
		let entries = [file(&staging, "a"), file(&staging, "b"), file(&staging, "c")];

		assert!(install(&staging, &entries, &[], &[], &root).is_err());

		assert_eq!(text(&root, "a"), "old a");
		assert_eq!(text(&root, "b"), "old b");
		assert!(!root.join("a.calcite-old").exists() && !root.join("b.calcite-old").exists());
		assert!(!root.join("c").exists());
		assert!(!root.join(JOURNAL).exists());
	}

	#[test]
	fn a_journal_left_by_a_crash_is_settled_on_the_next_start() {
		let temp = TempDir::new();
		let root = temp.0.join("root");
		fs::create_dir_all(&root).unwrap();
		// The crash came after `a` moved and its replacement arrived, before the commit.
		fs::write(root.join("a"), "new a").unwrap();
		fs::write(root.join("a.calcite-old"), "old a").unwrap();
		fs::write(root.join("unrelated"), "mine").unwrap();
		let journal = |committed| Journal {
			committed,
			directories: Vec::new(),
			operations: vec![Operation {
				path: "a".into(),
				aside: Some("a.calcite-old".into()),
				install: true,
				fingerprint: String::new(),
			}],
		};

		write(&root, &journal(false)).unwrap();
		recover(&root).unwrap();
		assert_eq!(text(&root, "a"), "old a");
		assert!(!root.join("a.calcite-old").exists() && !root.join(JOURNAL).exists());

		// A committed install only waits for its leftovers to go.
		fs::write(root.join("a"), "new a").unwrap();
		fs::write(root.join("a.calcite-old"), "old a").unwrap();
		write(&root, &journal(true)).unwrap();
		recover(&root).unwrap();
		assert_eq!(text(&root, "a"), "new a");
		assert!(!root.join("a.calcite-old").exists() && !root.join(JOURNAL).exists());
		assert_eq!(text(&root, "unrelated"), "mine");
	}

	#[test]
	fn a_journal_that_names_files_it_did_not_write_is_refused() {
		let temp = TempDir::new();
		let root = temp.0.join("root");
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("important.cfg"), "keep").unwrap();
		fs::write(root.join("a"), "keep").unwrap();
		let planted = |committed, path: &str, aside: Option<&str>| Journal {
			committed,
			directories: Vec::new(),
			operations: vec![Operation {
				path: path.into(),
				aside: aside.map(String::from),
				install: true,
				fingerprint: String::new(),
			}],
		};

		// A committed journal would delete its aside file; that is not `<path>.calcite-old`.
		write(&root, &planted(true, "a", Some("important.cfg"))).unwrap();
		assert!(recover(&root).is_err());
		// A rollback would delete `a`, which has no aside file and no fingerprint.
		write(&root, &planted(false, "a", None)).unwrap();
		assert!(recover(&root).is_err());

		assert_eq!(text(&root, "important.cfg"), "keep");
		assert_eq!(text(&root, "a"), "keep");
	}

	#[cfg(unix)]
	#[test]
	fn a_journal_path_through_a_link_is_refused() {
		let temp = TempDir::new();
		let (root, outside) = (temp.0.join("root"), temp.0.join("outside"));
		fs::create_dir_all(&root).unwrap();
		fs::create_dir_all(&outside).unwrap();
		std::os::unix::fs::symlink(&outside, root.join("a")).unwrap();
		fs::write(outside.join("b"), "new").unwrap();
		fs::write(outside.join("b.calcite-old"), "old").unwrap();
		let journal = Journal {
			committed: false,
			directories: Vec::new(),
			operations: vec![Operation {
				path: "a/b".into(),
				aside: Some("a/b.calcite-old".into()),
				install: true,
				fingerprint: String::new(),
			}],
		};
		write(&root, &journal).unwrap();

		assert!(recover(&root).is_err());

		assert_eq!(fs::read_to_string(outside.join("b")).unwrap(), "new");
		assert_eq!(fs::read_to_string(outside.join("b.calcite-old")).unwrap(), "old");
	}
}
