// SPDX-License-Identifier: MPL-2.0

//! [`keymap!`](crate::keymap!): key bindings grouped by key context.
//!
//! Bindings in a named context are inactive while a text input has focus, so typing "g" or
//! a space into a [`text_field`](crate::forms::text_field) never fires an app shortcut.

/// The key context of every text input (set by the editable text element).
pub const INPUT_CONTEXT: &str = gpui_ce_elements::editable_text::actions::DEFAULT_INPUT_CONTEXT;

/// The binding predicate for `context` that is false while a text input is focused.
pub fn scoped(context: &str) -> String {
	format!("{context} && !{INPUT_CONTEXT}")
}

/// Binds keys to actions, grouped by key context. `_` binds globally.
///
/// Named contexts get `&& !EditableText` appended, so the keys do not fire while a text
/// input has focus. Modifier chords you want to work while typing belong under `_`.
///
/// ```ignore
/// calcite::keymap!(cx, {
///     "Home" => { "space" => PrimaryAction, "g" => CycleGlass },
///     _ => { "cmd-q" => Quit, "ctrl-q" => Quit },
/// });
/// ```
#[macro_export]
macro_rules! keymap {
	($cx:expr, { $($context:tt => { $($key:literal => $action:expr),* $(,)? }),* $(,)? }) => {
		$cx.bind_keys(::std::vec![
			$($(::gpui::KeyBinding::new(
				$key,
				$action,
				$crate::keymap!(@context $context).as_deref(),
			)),*),*
		])
	};
	(@context _) => { ::std::option::Option::<::std::string::String>::None };
	(@context $context:literal) => { ::std::option::Option::Some($crate::keymap::scoped($context)) };
}

#[cfg(test)]
mod tests {
	use super::*;
	use gpui::{KeyBindingContextPredicate, KeyContext};

	fn active(stack: &[&str]) -> bool {
		let predicate = KeyBindingContextPredicate::parse(&scoped("Gallery")).unwrap();
		let contexts: Vec<KeyContext> = stack.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
		// The same matching the dispatcher uses: any ancestor depth may satisfy the predicate.
		predicate.depth_of(&contexts).is_some()
	}

	#[test]
	fn shortcuts_are_active_outside_text_inputs() {
		assert!(active(&["Gallery"]));
		assert!(active(&["Gallery", "Button"]));
	}

	#[test]
	fn shortcuts_are_inactive_while_a_text_input_is_focused() {
		assert!(!active(&["Gallery", INPUT_CONTEXT]));
	}
}
