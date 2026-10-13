// SPDX-License-Identifier: MPL-2.0

//! [`keymap!`](crate::keymap!): key bindings grouped by key context.

/// Binds keys to actions, grouped by key context. `_` binds globally.
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
			$($(::gpui::KeyBinding::new($key, $action, $crate::keymap!(@context $context))),*),*
		])
	};
	(@context _) => { ::std::option::Option::None };
	(@context $context:literal) => { ::std::option::Option::Some($context) };
}
