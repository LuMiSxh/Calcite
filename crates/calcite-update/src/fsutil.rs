// SPDX-License-Identifier: MPL-2.0

//! File helpers shared by staging, installing, and the update state.

use std::{
	fs,
	io::{self, Read, Write},
	path::Path,
};

use sha2::{Digest, Sha256};

use crate::sign::encode_hex;

/// Opens a regular file without following a link. The handle is checked against the path
/// after it opens, so a swap between the check and the open is caught.
pub(crate) fn open_regular(path: &Path) -> io::Result<fs::File> {
	let before = fs::symlink_metadata(path)?;
	if !before.is_file() {
		return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
	}
	let file = fs::File::open(path)?;
	let after = file.metadata()?;
	#[cfg(unix)]
	let same = {
		use std::os::unix::fs::MetadataExt;
		after.dev() == before.dev() && after.ino() == before.ino()
	};
	#[cfg(windows)]
	let same = {
		use std::os::windows::fs::MetadataExt;
		const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
		// A link swapped in after the check shows as a reparse point on the opened handle.
		after.is_file() && after.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
	};
	#[cfg(not(any(unix, windows)))]
	let same = after.is_file();
	if same {
		Ok(file)
	} else {
		Err(io::Error::new(
			io::ErrorKind::InvalidData,
			"the file changed while it opened",
		))
	}
}

/// The size and lowercase hex SHA-256 of what the open handle yields.
pub(crate) fn hash_reader(mut reader: impl Read) -> io::Result<(u64, String)> {
	let mut hasher = Sha256::new();
	let mut buffer = vec![0; 1 << 16];
	let mut size = 0u64;
	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		size += count as u64;
		hasher.update(&buffer[..count]);
	}
	Ok((size, encode_hex(&hasher.finalize())))
}

/// The size and SHA-256 of a regular file, read from one opened handle.
pub(crate) fn hash_file(path: &Path) -> io::Result<(u64, String)> {
	hash_reader(open_regular(path)?)
}

/// Fails unless `path` is a real directory, not a link to one.
#[cfg(feature = "client")]
pub(crate) fn ensure_real_dir(path: &Path) -> io::Result<()> {
	fs::create_dir_all(path)?;
	if fs::symlink_metadata(path)?.is_dir() {
		Ok(())
	} else {
		Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"not a real directory (is it a link?)",
		))
	}
}

/// Writes `bytes` to a temporary file next to `path`, syncs it, and renames it into place.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
	let name = path.file_name().unwrap_or_default().to_string_lossy();
	let temporary = path.with_file_name(format!("{name}.tmp"));
	let _ = fs::remove_file(&temporary);
	let mut file = fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&temporary)?;
	file.write_all(bytes)?;
	file.sync_all()?;
	drop(file);
	fs::rename(&temporary, path)?;
	#[cfg(unix)]
	if let Some(parent) = path.parent()
		&& let Ok(directory) = fs::File::open(parent)
	{
		let _ = directory.sync_all();
	}
	Ok(())
}

/// A temporary folder that unit tests own and remove.
#[cfg(test)]
pub(crate) struct TempDir(pub(crate) std::path::PathBuf);

#[cfg(test)]
impl TempDir {
	pub(crate) fn new() -> Self {
		use std::sync::atomic::{AtomicU32, Ordering};
		static COUNTER: AtomicU32 = AtomicU32::new(0);
		let count = COUNTER.fetch_add(1, Ordering::Relaxed);
		let path = std::env::temp_dir().join(format!("calcite-unit-{}-{count}", std::process::id()));
		let _ = fs::remove_dir_all(&path);
		fs::create_dir_all(&path).unwrap();
		Self(path)
	}
}

#[cfg(test)]
impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}
