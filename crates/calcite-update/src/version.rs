// SPDX-License-Identifier: MPL-2.0

//! Release versions: dotted numbers with an optional pre-release suffix (`1.4.0-canary.2`).

use std::{cmp::Ordering, fmt, str::FromStr};

/// A release version. A pre-release sorts before the release with the same numbers, and
/// pre-releases compare part by part, numbers numerically.
#[derive(Clone, Debug)]
pub struct Version {
	numbers: Vec<u64>,
	pre: Option<String>,
}

impl FromStr for Version {
	type Err = String;

	fn from_str(text: &str) -> Result<Self, Self::Err> {
		let text = text.trim().trim_start_matches('v');
		let (core, pre) = match text.split_once('-') {
			Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
			Some(_) => return Err(format!("version `{text}`: empty pre-release")),
			None => (text, None),
		};
		let numbers = core
			.split('.')
			.map(|part| {
				part.parse::<u64>()
					.map_err(|_| format!("version `{text}`: `{part}` is not a number"))
			})
			.collect::<Result<Vec<_>, _>>()?;
		if numbers.is_empty() {
			return Err(format!("version `{text}` is empty"));
		}
		Ok(Self { numbers, pre })
	}
}

impl fmt::Display for Version {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let numbers: Vec<String> = self.numbers.iter().map(u64::to_string).collect();
		write!(f, "{}", numbers.join("."))?;
		if let Some(pre) = &self.pre {
			write!(f, "-{pre}")?;
		}
		Ok(())
	}
}

impl Ord for Version {
	fn cmp(&self, other: &Self) -> Ordering {
		let length = self.numbers.len().max(other.numbers.len());
		for index in 0..length {
			let a = self.numbers.get(index).copied().unwrap_or(0);
			let b = other.numbers.get(index).copied().unwrap_or(0);
			match a.cmp(&b) {
				Ordering::Equal => {}
				order => return order,
			}
		}
		match (&self.pre, &other.pre) {
			(None, None) => Ordering::Equal,
			(None, Some(_)) => Ordering::Greater,
			(Some(_), None) => Ordering::Less,
			(Some(a), Some(b)) => compare_pre(a, b),
		}
	}
}

// `1.0` equals `1.0.0`, so equality follows the ordering, not the fields.
impl PartialEq for Version {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other) == Ordering::Equal
	}
}

impl Eq for Version {}

impl PartialOrd for Version {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

fn compare_pre(a: &str, b: &str) -> Ordering {
	let mut a = a.split('.');
	let mut b = b.split('.');
	loop {
		match (a.next(), b.next()) {
			(None, None) => return Ordering::Equal,
			(None, Some(_)) => return Ordering::Less,
			(Some(_), None) => return Ordering::Greater,
			(Some(x), Some(y)) => {
				let order = match (x.parse::<u64>(), y.parse::<u64>()) {
					(Ok(x), Ok(y)) => x.cmp(&y),
					_ => x.cmp(y),
				};
				if order != Ordering::Equal {
					return order;
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn versions_order_like_releases() {
		let ordered = [
			"0.9.9",
			"1.0.0-canary.2",
			"1.0.0-canary.10",
			"1.0.0",
			"1.0.1",
			"1.10.0",
			"v2",
		];
		let parsed: Vec<Version> = ordered.iter().map(|text| text.parse().unwrap()).collect();
		for pair in parsed.windows(2) {
			assert!(pair[0] < pair[1], "{} < {}", pair[0], pair[1]);
		}
		assert_eq!(
			"1.0".parse::<Version>().unwrap(),
			"1.0.0".parse::<Version>().unwrap()
		);
	}
}
