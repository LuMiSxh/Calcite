// SPDX-License-Identifier: MPL-2.0

//! The signed envelope around a feed. The signature covers the feed's exact bytes, so no
//! JSON canonicalization can weaken it.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{Error, Feed, Result};

/// `update.json`: the feed as text and its Ed25519 signature, both as published.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
	pub feed: String,
	/// Lowercase hex of the 64-byte signature.
	pub signature: String,
}

/// A public key in lowercase hex (64 characters), as apps embed it.
pub fn verifying_key(hex: &str) -> Result<VerifyingKey> {
	let bytes: [u8; 32] = decode_hex(hex)?
		.try_into()
		.map_err(|_| Error::Signature("the key is not 32 bytes".into()))?;
	VerifyingKey::from_bytes(&bytes).map_err(|error| Error::Signature(error.to_string()))
}

impl Envelope {
	/// Verifies the signature, then parses the feed. Nothing in the feed is read before.
	pub fn open(&self, key: &VerifyingKey) -> Result<Feed> {
		let bytes: [u8; 64] = decode_hex(&self.signature)?
			.try_into()
			.map_err(|_| Error::Signature("the signature is not 64 bytes".into()))?;
		key.verify_strict(self.feed.as_bytes(), &Signature::from_bytes(&bytes))
			.map_err(|_| Error::Signature("the feed signature does not match the app's key".into()))?;
		serde_json::from_str(&self.feed).map_err(|error| Error::Feed(error.to_string()))
	}
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
	bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn decode_hex(text: &str) -> Result<Vec<u8>> {
	let text = text.trim();
	if !text.len().is_multiple_of(2) {
		return Err(Error::Signature("odd hex length".into()));
	}
	(0..text.len())
		.step_by(2)
		.map(|index| {
			u8::from_str_radix(&text[index..index + 2], 16)
				.map_err(|_| Error::Signature("invalid hex".into()))
		})
		.collect()
}
