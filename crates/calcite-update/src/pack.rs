// SPDX-License-Identifier: MPL-2.0

//! The two package formats.
//!
//! - A **full** package is one zstd stream with every file of the tree, in tree order. The
//!   tree in the feed gives each file's size, so the stream needs no headers.
//! - A **delta** package holds only what changed. It starts with `CALCITE-DELTA 1\n`, a
//!   little-endian `u64` index length, and the JSON [`DeltaIndex`]. Then come the payloads,
//!   one zstd frame each: a file compressed alone, or a patch against the old file at
//!   `base` (zstd `--patch-from`). Files the delta does not name are unchanged and come
//!   from the installed copy.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

pub(crate) const DELTA_MAGIC: &[u8] = b"CALCITE-DELTA 1\n";
/// The largest index a client reads; a real one is a few kilobytes per thousand files.
pub(crate) const MAX_INDEX: u64 = 16 << 20;
/// zstd's window must cover the whole base for a patch; 2 GiB is its limit.
pub(crate) const MAX_PATCH_BASE: u64 = 1 << 31;
pub(crate) const LEVEL: i32 = 19;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeltaIndex {
	pub from: String,
	pub to: String,
	pub items: Vec<DeltaItem>,
}

/// One new or changed file. `length` is the size of its payload in the package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeltaItem {
	pub path: String,
	pub length: u64,
	#[serde(flatten)]
	pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum Source {
	/// The file compressed on its own.
	Whole,
	/// A patch from the installed file at `base`, which must hash to `base_sha256`.
	Patch { base: String, base_sha256: String },
}

/// The window log that covers `size` bytes, for patches.
pub(crate) fn window_log(size: u64) -> u32 {
	let size = size.max(1 << 10);
	(64 - (size - 1).leading_zeros()).clamp(10, 31)
}

/// Compresses `data` as a patch against `base`, or alone when `base` is `None`.
pub(crate) fn compress(data: &[u8], base: Option<&[u8]>) -> io::Result<Vec<u8>> {
	let mut out = Vec::new();
	match base {
		Some(base) => {
			let mut encoder = zstd::stream::write::Encoder::with_ref_prefix(&mut out, LEVEL, base)?;
			encoder.window_log(window_log(base.len().max(data.len()) as u64))?;
			encoder.long_distance_matching(true)?;
			encoder.include_checksum(true)?;
			encoder.write_all(data)?;
			encoder.finish()?;
		}
		None => {
			let mut encoder = zstd::stream::write::Encoder::new(&mut out, LEVEL)?;
			encoder.include_checksum(true)?;
			encoder.write_all(data)?;
			encoder.finish()?;
		}
	}
	Ok(out)
}

/// Decompresses one payload into `out`, reading at most `limit` bytes of output.
pub(crate) fn decompress(
	payload: impl Read,
	base: Option<&[u8]>,
	limit: u64,
	out: &mut dyn Write,
) -> io::Result<u64> {
	let written = match base {
		Some(base) => {
			let mut decoder =
				zstd::stream::read::Decoder::with_ref_prefix(io::BufReader::new(payload), base)?;
			decoder.window_log_max(31)?;
			io::copy(&mut (&mut decoder).take(limit + 1), out)?
		}
		None => {
			let mut decoder = zstd::stream::read::Decoder::new(payload)?;
			decoder.window_log_max(31)?;
			io::copy(&mut (&mut decoder).take(limit + 1), out)?
		}
	};
	if written > limit {
		return Err(io::Error::new(
			io::ErrorKind::InvalidData,
			"a payload is larger than its file",
		));
	}
	Ok(written)
}
