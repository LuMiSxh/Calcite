// SPDX-License-Identifier: MPL-2.0

//! Writing a release: the tree of a built app, its full package, deltas from older
//! releases, and the signed envelope.

use std::{
	collections::BTreeMap,
	fs,
	io::{self, Write},
	path::{Path, PathBuf},
};

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use crate::{
	Asset, Entry, Envelope, Error, Feed, Kind, Result,
	manifest::validate_entries,
	pack::{self, DeltaIndex, DeltaItem, Source},
	sign::{decode_hex, encode_hex},
	stage::sha256_file,
};

/// The tree of a built app, in the order a stager writes it (parents first). A file root,
/// such as an AppImage, becomes a tree of that one file.
pub fn scan(root: &Path) -> Result<Vec<Entry>> {
	let metadata = fs::symlink_metadata(root).map_err(|error| Error::io(root, error))?;
	if metadata.is_file() {
		let name = root
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		return Ok(vec![file_entry(root, name)?]);
	}
	let mut entries = Vec::new();
	walk(root, "", &mut entries)?;
	validate_entries(&entries)?;
	Ok(entries)
}

fn walk(directory: &Path, prefix: &str, entries: &mut Vec<Entry>) -> Result<()> {
	let mut children: Vec<_> = fs::read_dir(directory)
		.map_err(|error| Error::io(directory, error))?
		.collect::<io::Result<_>>()
		.map_err(|error| Error::io(directory, error))?;
	children.sort_by_key(|child| child.file_name());
	for child in children {
		let name = child.file_name().to_string_lossy().into_owned();
		let path = if prefix.is_empty() {
			name
		} else {
			format!("{prefix}/{name}")
		};
		let full = child.path();
		let kind = child.file_type().map_err(|error| Error::io(&full, error))?;
		if kind.is_symlink() {
			let target = fs::read_link(&full).map_err(|error| Error::io(&full, error))?;
			entries.push(Entry {
				path,
				kind: Kind::Symlink {
					target: target.to_string_lossy().replace('\\', "/"),
				},
			});
		} else if kind.is_dir() {
			entries.push(Entry {
				path: path.clone(),
				kind: Kind::Directory,
			});
			walk(&full, &path, entries)?;
		} else if kind.is_file() {
			entries.push(file_entry(&full, path)?);
		} else {
			return Err(Error::Feed(format!(
				"`{path}` is neither a file, a folder, nor a link"
			)));
		}
	}
	Ok(())
}

fn file_entry(full: &Path, path: String) -> Result<Entry> {
	let metadata = fs::metadata(full).map_err(|error| Error::io(full, error))?;
	#[cfg(unix)]
	let executable = {
		use std::os::unix::fs::PermissionsExt;
		metadata.permissions().mode() & 0o111 != 0
	};
	#[cfg(not(unix))]
	let executable = false;
	Ok(Entry {
		path,
		kind: Kind::File {
			size: metadata.len(),
			sha256: sha256_file(full).map_err(|error| Error::io(full, error))?,
			executable,
		},
	})
}

fn join(root: &Path, path: &str) -> PathBuf {
	if fs::metadata(root).is_ok_and(|metadata| metadata.is_file()) {
		return root.to_path_buf();
	}
	path.split('/')
		.fold(root.to_path_buf(), |path, part| path.join(part))
}

fn asset(path: &Path) -> Result<Asset> {
	Ok(Asset {
		name: path
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default(),
		size: fs::metadata(path).map_err(|error| Error::io(path, error))?.len(),
		sha256: sha256_file(path).map_err(|error| Error::io(path, error))?,
	})
}

/// Writes the full package of the tree at `root` to `out`.
pub fn write_full(root: &Path, entries: &[Entry], out: &Path) -> Result<Asset> {
	let file = fs::File::create(out).map_err(|error| Error::io(out, error))?;
	let mut encoder = zstd::stream::write::Encoder::new(io::BufWriter::new(file), pack::LEVEL)
		.map_err(|error| Error::io(out, error))?;
	encoder
		.include_checksum(true)
		.map_err(|error| Error::io(out, error))?;
	for entry in entries {
		if let Kind::File { .. } = entry.kind {
			let source = join(root, &entry.path);
			let mut input = fs::File::open(&source).map_err(|error| Error::io(&source, error))?;
			io::copy(&mut input, &mut encoder).map_err(|error| Error::io(&source, error))?;
		}
	}
	encoder
		.finish()
		.and_then(|mut writer| writer.flush())
		.map_err(|error| Error::io(out, error))?;
	asset(out)
}

/// Writes the delta from release `from` (tree `old_entries` at `old_root`) to release `to`.
/// Unchanged files are left out; a changed file becomes a patch when the old release had a
/// file at the same path, or with the same name, and the patch is smaller.
pub fn write_delta(
	old_root: &Path,
	old_entries: &[Entry],
	new_root: &Path,
	new_entries: &[Entry],
	from: &str,
	to: &str,
	out: &Path,
) -> Result<Asset> {
	let old_files: BTreeMap<&str, &str> = old_entries
		.iter()
		.filter_map(|entry| match &entry.kind {
			Kind::File { sha256, .. } => Some((entry.path.as_str(), sha256.as_str())),
			_ => None,
		})
		.collect();
	let by_name: BTreeMap<&str, &str> = old_files
		.keys()
		.map(|path| (path.rsplit('/').next().unwrap_or(path), *path))
		.collect();

	let mut items = Vec::new();
	let mut payloads = Vec::new();
	for entry in new_entries {
		let Kind::File { sha256, .. } = &entry.kind else {
			continue;
		};
		if old_files.get(entry.path.as_str()) == Some(&sha256.as_str()) {
			continue;
		}
		let data = fs::read(join(new_root, &entry.path)).map_err(|error| Error::io(new_root, error))?;
		let whole = pack::compress(&data, None).map_err(Error::Io)?;
		let name = entry.path.rsplit('/').next().unwrap_or(&entry.path);
		let base_path = old_files
			.contains_key(entry.path.as_str())
			.then_some(entry.path.as_str())
			.or_else(|| by_name.get(name).copied());
		let mut chosen = (Source::Whole, whole);
		if let Some(base_path) = base_path {
			let base_full = join(old_root, base_path);
			let base_size = fs::metadata(&base_full)
				.map_err(|error| Error::io(&base_full, error))?
				.len();
			if base_size <= pack::MAX_PATCH_BASE {
				let base = fs::read(&base_full).map_err(|error| Error::io(&base_full, error))?;
				let patch = pack::compress(&data, Some(&base)).map_err(Error::Io)?;
				if patch.len() < chosen.1.len() {
					let source = Source::Patch {
						base: base_path.to_string(),
						base_sha256: encode_hex(&Sha256::digest(&base)),
					};
					chosen = (source, patch);
				}
			}
		}
		items.push(DeltaItem {
			path: entry.path.clone(),
			length: chosen.1.len() as u64,
			source: chosen.0,
		});
		payloads.push(chosen.1);
	}

	let index = serde_json::to_vec(&DeltaIndex {
		from: from.to_string(),
		to: to.to_string(),
		items,
	})
	.map_err(|error| Error::Feed(error.to_string()))?;
	let mut file = io::BufWriter::new(fs::File::create(out).map_err(|error| Error::io(out, error))?);
	let write = |file: &mut io::BufWriter<fs::File>| -> io::Result<()> {
		file.write_all(pack::DELTA_MAGIC)?;
		file.write_all(&(index.len() as u64).to_le_bytes())?;
		file.write_all(&index)?;
		for payload in &payloads {
			file.write_all(payload)?;
		}
		file.flush()
	};
	write(&mut file).map_err(|error| Error::io(out, error))?;
	drop(file);
	asset(out)
}

/// A new signing key as hex: (secret seed, public key). Keep the seed out of the repository.
pub fn generate_key() -> Result<(String, String)> {
	let mut seed = [0u8; 32];
	getrandom::fill(&mut seed).map_err(|error| Error::Signature(error.to_string()))?;
	let key = SigningKey::from_bytes(&seed);
	Ok((encode_hex(&seed), encode_hex(key.verifying_key().as_bytes())))
}

/// The signing key from a hex seed, as `generate_key` prints it.
pub fn signing_key(seed_hex: &str) -> Result<SigningKey> {
	let seed: [u8; 32] = decode_hex(seed_hex)?
		.try_into()
		.map_err(|_| Error::Signature("the seed is not 32 bytes".into()))?;
	Ok(SigningKey::from_bytes(&seed))
}

/// Signs `feed` into the envelope clients download as `update.json`.
pub fn sign(feed: &Feed, key: &SigningKey) -> Result<Envelope> {
	let text = serde_json::to_string_pretty(feed).map_err(|error| Error::Feed(error.to_string()))?;
	let signature = key.sign(text.as_bytes());
	Ok(Envelope {
		feed: text,
		signature: encode_hex(&signature.to_bytes()),
	})
}
