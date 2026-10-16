// SPDX-License-Identifier: MPL-2.0

//! Staging: every packager run writes into its own directory under one staging root. The
//! outputs move into the output directory only after all runs succeed, so a failed run leaves
//! the output directory as it was. The final move is all-or-nothing as far as renames allow:
//! old outputs go aside first, and a failure puts everything back.

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

	/// Moves each staged output into `out_dir`, then removes the root. Existing targets move into
	/// the root first; if any rename fails, the new outputs move back out and the old ones return.
	/// The root is removed either way, and a cleanup failure is logged.
	pub fn commit(self, out_dir: &Path, outputs: Vec<Output>) -> Result<Vec<Output>> {
		let result = self.swap(out_dir, outputs);
		if let Err(cleanup) = self.discard() {
			eprintln!("warning: {cleanup}");
		}
		result
	}

	fn swap(&self, out_dir: &Path, outputs: Vec<Output>) -> Result<Vec<Output>> {
		fs::create_dir_all(out_dir).map_err(|error| format!("{}: {error}", out_dir.display()))?;
		let mut moves = Vec::new();
		let mut published = Vec::with_capacity(outputs.len());
		for output in outputs {
			let mut paths = Vec::with_capacity(output.paths.len());
			for staged in output.paths {
				let name = staged
					.file_name()
					.ok_or_else(|| format!("{}: no file name", staged.display()))?;
				let target = out_dir.join(name);
				if moves.iter().any(|(_, existing)| *existing == target) {
					return Err(format!("{}: two outputs have this name", target.display()));
				}
				moves.push((staged, target.clone()));
				paths.push(target);
			}
			published.push(Output {
				format: output.format,
				paths,
			});
		}

		let old_dir = self.root.join(".old");
		fs::create_dir_all(&old_dir).map_err(|error| format!("{}: {error}", old_dir.display()))?;
		let mut aside: Vec<(PathBuf, PathBuf)> = Vec::new();
		let mut installed: Vec<&(PathBuf, PathBuf)> = Vec::new();
		let attempt = (|| -> Result<()> {
			for (index, (_, target)) in moves.iter().enumerate() {
				match fs::symlink_metadata(target) {
					Ok(_) => {
						let old = old_dir.join(index.to_string());
						rename(target, &old)?;
						aside.push((old, target.clone()));
					}
					Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
					Err(error) => return Err(format!("{}: {error}", target.display())),
				}
			}
			for entry in &moves {
				rename(&entry.0, &entry.1)?;
				installed.push(entry);
			}
			Ok(())
		})();
		match attempt {
			Ok(()) => Ok(published),
			Err(error) => {
				let mut problems = vec![error];
				for (staged, target) in installed.into_iter().rev() {
					problems.extend(rename(target, staged).err());
				}
				for (old, target) in aside.iter().rev() {
					problems.extend(rename(old, target).err());
				}
				Err(problems.join("\n"))
			}
		}
	}

	/// Removes the staging root and everything in it.
	pub fn discard(&self) -> Result<()> {
		match fs::remove_dir_all(&self.root) {
			Ok(()) => Ok(()),
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
			Err(error) => Err(format!("{}: {error}", self.root.display())),
		}
	}
}

fn rename(from: &Path, to: &Path) -> Result<()> {
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

	#[test]
	fn a_failed_commit_restores_the_output_directory_and_removes_the_stage() {
		let bundle = temp("rollback");
		let out = bundle.join("release");
		fs::create_dir_all(out.join("App.app")).unwrap();
		fs::write(out.join("App.app/info"), b"old app").unwrap();
		fs::write(out.join("App.deb"), b"old deb").unwrap();
		let staging = Staging::create(&bundle).unwrap();
		let run = staging.dir("run-0").unwrap();
		let deb = run.join("App.deb");
		fs::write(&deb, b"new deb").unwrap();
		// This staged output is missing, so its rename fails after the first one succeeded.
		let missing = run.join("App.app");
		let result = staging.commit(
			&out,
			vec![Output {
				format: "mixed".to_string(),
				paths: vec![deb, missing],
			}],
		);
		assert!(result.is_err());
		assert_eq!(fs::read(out.join("App.deb")).unwrap(), b"old deb");
		assert_eq!(fs::read(out.join("App.app/info")).unwrap(), b"old app");
		assert_eq!(fs::read_dir(&out).unwrap().count(), 2);
		assert!(!bundle.join(format!(".staging-{}", std::process::id())).exists());
		fs::remove_dir_all(&bundle).unwrap();
	}
}
