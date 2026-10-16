// SPDX-License-Identifier: MPL-2.0

//! The highest feed sequence a client has verified, so an old signed feed cannot replay.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, fsutil};

const STATE_FILE: &str = "update-state.json";

#[derive(Default, Serialize, Deserialize)]
struct State {
	highest_sequence: u64,
}

/// The highest sequence recorded in `directory`; zero before the first update check.
pub(crate) fn highest_sequence(directory: &Path) -> Result<u64> {
	let path = directory.join(STATE_FILE);
	match fs::read_to_string(&path) {
		Ok(text) => serde_json::from_str::<State>(&text)
			.map(|state| state.highest_sequence)
			.map_err(|error| Error::Feed(format!("{}: {error}", path.display()))),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
		Err(error) => Err(Error::io(&path, error)),
	}
}

/// Raises the recorded sequence to `sequence`. The write is atomic and synced.
pub(crate) fn record(directory: &Path, sequence: u64) -> Result<()> {
	if sequence <= highest_sequence(directory)? {
		return Ok(());
	}
	fsutil::ensure_real_dir(directory).map_err(|error| Error::io(directory, error))?;
	let path = directory.join(STATE_FILE);
	let text = serde_json::to_vec(&State {
		highest_sequence: sequence,
	})
	.map_err(|error| Error::Feed(error.to_string()))?;
	fsutil::write_atomic(&path, &text).map_err(|error| Error::io(&path, error))
}
