// SPDX-License-Identifier: MPL-2.0

//! Staging: every packager run writes into its own directory under one staging root. The
//! outputs move into the output directory only after all runs succeed, so a failed run leaves
//! the output directory as it was.

use std::{
	fs,
	path::{Path, PathBuf},
};

use crate::package::{Output, Result};

/// The staging root, a sibling of the output directory on the same filesystem.
pub struct Staging {
	root: PathBuf,
}

impl Staging {
	/// Creates a fresh staging root under `bundle_dir`, named for this process.
	pub fn create(bundle_dir: &Path) -> Result<Self> {
		let root = bundle_dir.join(format!(".staging-{}", std::process::id()));
		// A leftover from an earlier crashed run with the same pid would mix in old files.
		if root.exists() {
			fs::remove_dir_all(&root).map_err(|error| format!("{}: {error}", root.display()))?;
		}
		fs::create_dir_all(&root).map_err(|error| format!("{}: {error}", root.display()))?;
		Ok(Self { root })
	}

	/// A directory under the staging root, created on first use. Each packager run has its own.
	pub fn dir(&self, name: &str) -> Result<PathBuf> {
		let dir = self.root.join(name);
		fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
		Ok(dir)
	}

	/// Moves each staged output into `out_dir`, one rename per file, then removes the root.
	pub fn commit(self, out_dir: &Path, outputs: Vec<Output>) -> Result<Vec<Output>> {
		fs::create_dir_all(out_dir).map_err(|error| format!("{}: {error}", out_dir.display()))?;
		let mut published = Vec::with_capacity(outputs.len());
		for output in outputs {
			let mut paths = Vec::with_capacity(output.paths.len());
			for staged in output.paths {
				let name = staged
					.file_name()
					.ok_or_else(|| format!("{}: no file name", staged.display()))?;
				let target = out_dir.join(name);
				replace(&staged, &target)?;
				paths.push(target);
			}
			published.push(Output {
				format: output.format,
				paths,
			});
		}
		self.discard()?;
		Ok(published)
	}

	/// Removes the staging root and everything in it.
	pub fn discard(self) -> Result<()> {
		match fs::remove_dir_all(&self.root) {
			Ok(()) => Ok(()),
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
			Err(error) => Err(format!("{}: {error}", self.root.display())),
		}
	}
}

/// Renames `from` over `to`, removing whatever `to` held: a file, or a directory such as `.app`.
fn replace(from: &Path, to: &Path) -> Result<()> {
	match fs::symlink_metadata(to) {
		Ok(metadata) if metadata.is_dir() => {
			fs::remove_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?
		}
		Ok(_) => fs::remove_file(to).map_err(|error| format!("{}: {error}", to.display()))?,
		Err(_) => {}
	}
	fs::rename(from, to).map_err(|error| format!("moving {} to {}: {error}", from.display(), to.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn temp(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("calcite-stage-{}-{name}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		dir
	}

	#[test]
	fn a_discarded_stage_leaves_the_output_directory_untouched() {
		let bundle = temp("discard");
		let out = bundle.join("release");
		fs::create_dir_all(&out).unwrap();
		let staging = Staging::create(&bundle).unwrap();
		fs::write(staging.dir("run-0").unwrap().join("App.deb"), b"new").unwrap();
		staging.discard().unwrap();
		assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
		assert!(!bundle.join(format!(".staging-{}", std::process::id())).exists());
		fs::remove_dir_all(&bundle).unwrap();
	}

	#[test]
	fn a_committed_stage_replaces_the_previous_outputs() {
		let bundle = temp("commit");
		let out = bundle.join("release");
		fs::create_dir_all(out.join("App.app")).unwrap();
		fs::write(out.join("App.deb"), b"old").unwrap();
		let staging = Staging::create(&bundle).unwrap();
		let deb = staging.dir("run-0").unwrap().join("App.deb");
		fs::write(&deb, b"new").unwrap();
		let published = staging
			.commit(
				&out,
				vec![Output {
					format: "deb".to_string(),
					paths: vec![deb],
				}],
			)
			.unwrap();
		assert_eq!(fs::read(&published[0].paths[0]).unwrap(), b"new");
		assert_eq!(published[0].paths[0], out.join("App.deb"));
		fs::remove_dir_all(&bundle).unwrap();
	}
}
