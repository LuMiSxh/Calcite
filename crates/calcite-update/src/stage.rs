// SPDX-License-Identifier: MPL-2.0

//! Builds the new tree in a fresh staging directory. Every file is hashed while it is
//! written and must match the signed tree; the installed copy is only read.

use std::{
	collections::BTreeMap,
	fs,
	io::{self, Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
	Entry, Error, Kind, Result,
	manifest::{validate_path, validate_sha},
	pack::{self, DeltaIndex, Source},
	sign::encode_hex,
};

/// A writer that hashes and counts what passes through it.
struct Hashing<W> {
	inner: W,
	hasher: Sha256,
	written: u64,
}

impl<W: Write> Write for Hashing<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let count = self.inner.write(buf)?;
		self.hasher.update(&buf[..count]);
		self.written += count as u64;
		Ok(count)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.inner.flush()
	}
}

pub(crate) fn sha256_file(path: &Path) -> io::Result<String> {
	let mut hasher = Sha256::new();
	let mut file = fs::File::open(path)?;
	let mut buffer = vec![0; 1 << 16];
	loop {
		let count = file.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		hasher.update(&buffer[..count]);
	}
	Ok(encode_hex(&hasher.finalize()))
}

/// Creates `staging` empty. An earlier attempt is removed first.
fn fresh(staging: &Path) -> Result<()> {
	if fs::symlink_metadata(staging).is_ok() {
		fs::remove_dir_all(staging).map_err(|error| Error::io(staging, error))?;
	}
	fs::create_dir_all(staging).map_err(|error| Error::io(staging, error))
}

fn join(root: &Path, path: &str) -> PathBuf {
	path.split('/')
		.fold(root.to_path_buf(), |path, part| path.join(part))
}

/// Writes one file of the tree by running `fill`, then checks its size and hash.
fn write_file(
	staging: &Path,
	entry: &Entry,
	fill: impl FnOnce(&mut dyn Write, u64) -> io::Result<()>,
) -> Result<()> {
	let Kind::File {
		size,
		sha256,
		executable,
	} = &entry.kind
	else {
		unreachable!("write_file takes files");
	};
	let path = join(staging, &entry.path);
	let file = fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&path)
		.map_err(|error| Error::io(&path, error))?;
	let mut writer = Hashing {
		inner: io::BufWriter::new(file),
		hasher: Sha256::new(),
		written: 0,
	};
	fill(&mut writer, *size).map_err(|error| Error::io(&path, error))?;
	writer.flush().map_err(|error| Error::io(&path, error))?;
	let actual = encode_hex(&writer.hasher.clone().finalize());
	if writer.written != *size || actual != *sha256 {
		return Err(Error::Integrity(format!(
			"`{}` does not match the signed tree",
			entry.path
		)));
	}
	set_executable(&path, *executable)
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> Result<()> {
	use std::os::unix::fs::PermissionsExt;
	let mode = if executable { 0o755 } else { 0o644 };
	fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| Error::io(path, error))
}

#[cfg(not(unix))]
fn set_executable(_: &Path, _: bool) -> Result<()> {
	Ok(())
}

fn make_directory(staging: &Path, entry: &Entry) -> Result<()> {
	let path = join(staging, &entry.path);
	fs::create_dir(&path).map_err(|error| Error::io(&path, error))
}

fn make_link(staging: &Path, entry: &Entry, target: &str) -> Result<()> {
	let path = join(staging, &entry.path);
	#[cfg(unix)]
	return std::os::unix::fs::symlink(target, &path).map_err(|error| Error::io(&path, error));
	#[cfg(not(unix))]
	{
		let _ = target;
		Err(Error::Feed(format!(
			"`{}`: symlinks are not supported on this platform",
			path.display()
		)))
	}
}

/// Stages the tree from a full package.
pub fn stage_full(package: &Path, entries: &[Entry], staging: &Path) -> Result<()> {
	fresh(staging)?;
	let file = fs::File::open(package).map_err(|error| Error::io(package, error))?;
	let mut stream = zstd::stream::read::Decoder::new(file).map_err(|error| Error::io(package, error))?;
	for entry in entries {
		match &entry.kind {
			Kind::Directory => make_directory(staging, entry)?,
			Kind::Symlink { target } => make_link(staging, entry, target)?,
			Kind::File { .. } => write_file(staging, entry, |out, size| {
				let copied = io::copy(&mut (&mut stream).take(size), out)?;
				if copied == size {
					Ok(())
				} else {
					Err(io::Error::new(
						io::ErrorKind::UnexpectedEof,
						"the package ends early",
					))
				}
			})?,
		}
	}
	let mut rest = [0u8; 1];
	if stream
		.read(&mut rest)
		.map_err(|error| Error::io(package, error))?
		!= 0
	{
		return Err(Error::Integrity("the package holds more than the tree".into()));
	}
	Ok(())
}

/// Reads and checks the index of a delta package. Returns it with the payload offset.
pub fn read_delta_index(package: &mut fs::File) -> Result<(DeltaIndex, u64)> {
	let mut magic = vec![0; pack::DELTA_MAGIC.len()];
	package.read_exact(&mut magic).map_err(Error::Io)?;
	if magic != pack::DELTA_MAGIC {
		return Err(Error::Integrity("not a delta package".into()));
	}
	let mut length = [0u8; 8];
	package.read_exact(&mut length).map_err(Error::Io)?;
	let length = u64::from_le_bytes(length);
	if length > pack::MAX_INDEX {
		return Err(Error::Integrity("the delta index is too large".into()));
	}
	let mut index = vec![0; length as usize];
	package.read_exact(&mut index).map_err(Error::Io)?;
	let index: DeltaIndex =
		serde_json::from_slice(&index).map_err(|error| Error::Integrity(error.to_string()))?;
	Ok((index, (pack::DELTA_MAGIC.len() + 8) as u64 + length))
}

/// Stages the tree from a delta package and the installed tree at `installed`. Fails with
/// [`Error::Integrity`] when the installed copy does not match what the delta expects;
/// the caller then falls back to the full package.
pub fn stage_delta(package: &Path, entries: &[Entry], installed: &Path, staging: &Path) -> Result<()> {
	let mut file = fs::File::open(package).map_err(|error| Error::io(package, error))?;
	let (index, start) = read_delta_index(&mut file)?;
	let files: BTreeMap<&str, &Entry> = entries
		.iter()
		.filter(|entry| matches!(entry.kind, Kind::File { .. }))
		.map(|entry| (entry.path.as_str(), entry))
		.collect();
	let mut items = BTreeMap::new();
	let mut offset = start;
	for item in &index.items {
		validate_path(&item.path)?;
		if !files.contains_key(item.path.as_str()) {
			return Err(Error::Integrity(format!(
				"the delta names `{}`, which the tree lacks",
				item.path
			)));
		}
		if let Source::Patch { base, base_sha256 } = &item.source {
			validate_path(base)?;
			validate_sha(base_sha256)?;
		}
		if items.insert(item.path.as_str(), (item, offset)).is_some() {
			return Err(Error::Integrity(format!("the delta names `{}` twice", item.path)));
		}
		offset += item.length;
	}
	let package_size = file.metadata().map_err(Error::Io)?.len();
	if offset != package_size {
		return Err(Error::Integrity(
			"the delta payloads do not fill the package".into(),
		));
	}

	fresh(staging)?;
	for entry in entries {
		match &entry.kind {
			Kind::Directory => make_directory(staging, entry)?,
			Kind::Symlink { target } => make_link(staging, entry, target)?,
			Kind::File { .. } => match items.get(entry.path.as_str()) {
				Some((item, offset)) => {
					let base = match &item.source {
						Source::Whole => None,
						Source::Patch { base, base_sha256 } => Some(read_base(installed, base, base_sha256)?),
					};
					file.seek(SeekFrom::Start(*offset)).map_err(Error::Io)?;
					let payload = (&mut file).take(item.length);
					write_file(staging, entry, |out, size| {
						pack::decompress(payload, base.as_deref(), size, out).map(|_| ())
					})?;
				}
				None => {
					let source = installed_file(installed, &entry.path)?;
					write_file(staging, entry, |out, size| {
						let mut input = fs::File::open(&source)?;
						let copied = io::copy(&mut (&mut input).take(size + 1), out)?;
						if copied == size {
							Ok(())
						} else {
							Err(io::Error::new(
								io::ErrorKind::InvalidData,
								"the installed file changed size",
							))
						}
					})
					.map_err(|error| match error {
						Error::Integrity(_) => Error::Integrity(format!(
							"the installed `{}` differs from the release it claims",
							entry.path
						)),
						other => other,
					})?;
				}
			},
		}
	}
	Ok(())
}

/// The installed regular file at `path`. A link or a missing file means the installed copy
/// is not the release the delta starts from.
fn installed_file(installed: &Path, path: &str) -> Result<PathBuf> {
	let full = join(installed, path);
	match fs::symlink_metadata(&full) {
		Ok(metadata) if metadata.is_file() => Ok(full),
		_ => Err(Error::Integrity(format!("the installed copy lacks `{path}`"))),
	}
}

fn read_base(installed: &Path, path: &str, sha256: &str) -> Result<Vec<u8>> {
	let full = installed_file(installed, path)?;
	let size = fs::metadata(&full)
		.map_err(|error| Error::io(&full, error))?
		.len();
	if size > pack::MAX_PATCH_BASE {
		return Err(Error::Integrity(format!(
			"the base `{path}` is too large to patch"
		)));
	}
	let data = fs::read(&full).map_err(|error| Error::io(&full, error))?;
	if encode_hex(&Sha256::digest(&data)) != sha256 {
		return Err(Error::Integrity(format!(
			"the installed `{path}` is not the base the delta expects"
		)));
	}
	Ok(data)
}
