// SPDX-License-Identifier: MPL-2.0

//! `calcite-update`: writes and signs releases for calcite-update clients.

use std::{
	collections::BTreeMap,
	fs,
	io::Read,
	path::{Path, PathBuf},
	process::ExitCode,
};

use calcite_update::{Envelope, Feed, publish, stage, target_id};

const USAGE: &str = "\
Writes and signs releases for calcite-update clients.

Usage:
  calcite-update keygen
      Prints a new signing seed (keep it secret) and its public key.

  calcite-update release --product ID --channel NAME --version X.Y.Z --sequence N --bundle PATH
                         --out DIR [--name APP] [--target ID] [--notes FILE]
                         [--minimum-version X.Y.Z] [--previous X.Y.Z=PATH]...
      Writes the full package and one delta per --previous tree into DIR, and adds the
      target to DIR/feed.json (created when missing). Run it once per target. --sequence
      must be higher than the sequence of every feed you signed before.

  calcite-update sign --feed DIR/feed.json --out DIR/update.json --key-id ID
      Signs the feed and stamps it with the current time. The seed comes from
      CALCITE_UPDATE_KEY or standard input.

  calcite-update extract --feed FILE --package FILE --into DIR [--target ID]
      Rebuilds an earlier release's tree from its full package, for deltas.";

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let result = match args.first().map(String::as_str) {
		Some("keygen") => keygen(),
		Some("release") => Flags::parse(&args[1..]).and_then(|flags| release(&flags)),
		Some("sign") => Flags::parse(&args[1..]).and_then(|flags| sign(&flags)),
		Some("extract") => Flags::parse(&args[1..]).and_then(|flags| extract(&flags)),
		Some("-h" | "--help") | None => {
			println!("{USAGE}");
			return ExitCode::SUCCESS;
		}
		Some(other) => Err(format!("unknown command `{other}`")),
	};
	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("error: {error}\n\n{USAGE}");
			ExitCode::FAILURE
		}
	}
}

#[derive(Default)]
struct Flags(BTreeMap<String, Vec<String>>);

impl Flags {
	fn parse(args: &[String]) -> Result<Self, String> {
		let mut flags = Self::default();
		let mut args = args.iter();
		while let Some(flag) = args.next() {
			let name = flag
				.strip_prefix("--")
				.ok_or_else(|| format!("unexpected `{flag}`"))?;
			let value = args.next().ok_or_else(|| format!("--{name} needs a value"))?;
			flags.0.entry(name.to_string()).or_default().push(value.clone());
		}
		Ok(flags)
	}

	fn get(&self, name: &str) -> Option<&str> {
		self.0
			.get(name)
			.and_then(|values| values.last())
			.map(String::as_str)
	}

	fn require(&self, name: &str) -> Result<&str, String> {
		self.get(name).ok_or_else(|| format!("--{name} is required"))
	}

	fn all(&self, name: &str) -> &[String] {
		self.0.get(name).map_or(&[], Vec::as_slice)
	}
}

fn keygen() -> Result<(), String> {
	let (seed, public_key) = publish::generate_key().map_err(|error| error.to_string())?;
	eprintln!("Store the seed as a secret (CALCITE_UPDATE_KEY). Embed the public key in the app.");
	println!("seed:       {seed}");
	println!("public key: {public_key}");
	Ok(())
}

fn read_feed(path: &Path) -> Result<Feed, String> {
	let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
	// An envelope's feed text, or a bare feed.
	let text = match serde_json::from_str::<Envelope>(&text) {
		Ok(envelope) => envelope.feed,
		Err(_) => text,
	};
	serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn release(flags: &Flags) -> Result<(), String> {
	let product = flags.require("product")?;
	let channel = flags.require("channel")?;
	let version = flags.require("version")?;
	version.parse::<calcite_update::Version>()?;
	let sequence: u64 = flags
		.require("sequence")?
		.parse()
		.map_err(|_| "--sequence takes a whole number")?;
	let minimum_version = flags.get("minimum-version");
	if let Some(minimum) = minimum_version {
		minimum.parse::<calcite_update::Version>()?;
	}
	let bundle = PathBuf::from(flags.require("bundle")?);
	let out = PathBuf::from(flags.require("out")?);
	let target = flags.get("target").map_or_else(target_id, str::to_string);
	let name = flags.get("name").unwrap_or("app");
	let mut previous = Vec::new();
	for argument in flags.all("previous") {
		let (from, root) = argument.split_once('=').ok_or("--previous takes VERSION=PATH")?;
		previous.push((from.to_string(), PathBuf::from(root)));
	}

	let built = publish::build_target(&bundle, &out, name, version, &target, &previous)
		.map_err(|error| error.to_string())?;
	println!("full   {} ({} bytes)", built.full.name, built.full.size);
	for delta in &built.deltas {
		println!("delta  {} ({} bytes)", delta.asset.name, delta.asset.size);
	}

	let feed_path = out.join("feed.json");
	let mut feed = if feed_path.exists() {
		read_feed(&feed_path)?
	} else {
		publish::FeedBuilder::new(product, channel, version, sequence, publish::now()).build()
	};
	if feed.product != product
		|| feed.channel != channel
		|| feed.version != version
		|| feed.sequence != sequence
	{
		return Err(format!(
			"{} is for another product, channel, version, or sequence",
			feed_path.display()
		));
	}
	if let Some(minimum) = minimum_version {
		feed.minimum_version = Some(minimum.to_string());
	}
	if let Some(notes) = flags.get("notes") {
		feed.notes = fs::read_to_string(notes).map_err(|error| format!("{notes}: {error}"))?;
	}
	feed.targets.insert(target, built);
	let text = serde_json::to_string_pretty(&feed).map_err(|error| error.to_string())?;
	fs::write(&feed_path, text).map_err(|error| format!("{}: {error}", feed_path.display()))?;
	println!("feed   {}", feed_path.display());
	Ok(())
}

fn sign(flags: &Flags) -> Result<(), String> {
	let mut feed = read_feed(Path::new(flags.require("feed")?))?;
	let out = PathBuf::from(flags.require("out")?);
	let key_id = flags.require("key-id")?;
	feed.issued_at = publish::now();
	// The seed never travels in arguments, which other processes can read.
	let seed = match std::env::var("CALCITE_UPDATE_KEY") {
		Ok(seed) => seed,
		Err(_) => {
			let mut seed = String::new();
			std::io::stdin()
				.read_to_string(&mut seed)
				.map_err(|error| error.to_string())?;
			seed
		}
	};
	let key = publish::signing_key(seed.trim()).map_err(|error| error.to_string())?;
	let bytes = publish::sign_feed(&feed, key_id, &key).map_err(|error| error.to_string())?;
	fs::write(&out, bytes).map_err(|error| format!("{}: {error}", out.display()))?;
	println!("signed {}", out.display());
	Ok(())
}

fn extract(flags: &Flags) -> Result<(), String> {
	let feed = read_feed(Path::new(flags.require("feed")?))?;
	let target = flags.get("target").map_or_else(target_id, str::to_string);
	let entries = &feed
		.targets
		.get(&target)
		.ok_or_else(|| format!("the feed has no target `{target}`"))?
		.entries;
	calcite_update::validate_entries(entries).map_err(|error| error.to_string())?;
	let into = PathBuf::from(flags.require("into")?);
	stage::stage_full(Path::new(flags.require("package")?), entries, &into)
		.map_err(|error| error.to_string())?;
	println!("extracted {} {} into {}", feed.version, target, into.display());
	Ok(())
}
