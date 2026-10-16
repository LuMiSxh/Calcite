// SPDX-License-Identifier: MPL-2.0

//! Keyboard routing: Tab, Return, and Escape reach the focused control or surface.

use std::rc::Rc;

use gpui::{
	AnyWindowHandle, App, ElementId, Entity, FocusHandle, Global, KeyBinding, KeyDownEvent, WeakEntity,
	Window, actions,
};

actions!(
	calcite,
	[
		/// Moves keyboard focus to the next control.
		FocusNext,
		/// Moves keyboard focus to the previous control.
		FocusPrevious,
	]
);

impl KeyRole {
	/// The keyed-state name of a slot with this role.
	fn slot_key(self) -> &'static str {
		match self {
			KeyRole::DefaultAction => "calcite-default-action",
			KeyRole::Dismiss => "calcite-dismiss",
			KeyRole::PassReturn => "calcite-pass-return",
			KeyRole::TabScope => "calcite-tab-scope",
		}
	}
}

/// What a key target answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRole {
	/// Return when no control that consumes Return has focus (`.defaultAction`).
	DefaultAction,
	/// Escape (`.cancelAction`): closes the topmost menu or modal.
	Dismiss,
	/// A focusable surface that passes Return on to the default action, like a modal
	/// container or a single-line text field.
	PassReturn,
	/// A modal's focus scope: Tab and Shift-Tab cycle inside it.
	TabScope,
}

pub(crate) type KeyHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// One registered target, kept as keyed state so it lives only while its control renders.
struct KeySlot {
	handler: Option<KeyHandler>,
	focus: Option<FocusHandle>,
}

#[derive(Default)]
struct KeyTargets(Vec<(AnyWindowHandle, KeyRole, WeakEntity<KeySlot>)>);

impl Global for KeyTargets {}

struct KeyboardInstalled;

impl Global for KeyboardInstalled {}

/// Registers the key target of the control `id` for this frame. Targets rendered later sit
/// on top, so an open menu closes before the modal under it.
pub(crate) fn register_key_target(
	id: &ElementId,
	role: KeyRole,
	handler: Option<KeyHandler>,
	focus: Option<FocusHandle>,
	window: &mut Window,
	cx: &mut App,
) {
	let slot = window.use_keyed_state((id.clone(), role.slot_key()), cx, |_, _| KeySlot {
		handler: None,
		focus: None,
	});
	slot.update(cx, |slot, _| {
		slot.handler = handler;
		slot.focus = focus;
	});
	let weak = slot.downgrade();
	let window_handle = window.window_handle();
	let targets = &mut cx.default_global::<KeyTargets>().0;
	targets.retain(|(_, _, slot)| slot.entity_id() != weak.entity_id() && slot.upgrade().is_some());
	targets.push((window_handle, role, weak));
}

/// Removes the key target of the control `id` for `role`, so a surface that is leaving (or gone)
/// stops answering keys.
pub(crate) fn unregister_key_target(id: &ElementId, role: KeyRole, window: &mut Window, cx: &mut App) {
	let slot = window.use_keyed_state((id.clone(), role.slot_key()), cx, |_, _| KeySlot {
		handler: None,
		focus: None,
	});
	let gone = slot.entity_id();
	if cx.has_global::<KeyTargets>() {
		cx.global_mut::<KeyTargets>()
			.0
			.retain(|(_, _, slot)| slot.entity_id() != gone);
	}
}

fn live_slots(role: KeyRole, window: &Window, cx: &App) -> Vec<Entity<KeySlot>> {
	let window_handle = window.window_handle();
	cx.try_global::<KeyTargets>()
		.map(|targets| {
			targets
				.0
				.iter()
				.filter(|(handle, slot_role, _)| *handle == window_handle && *slot_role == role)
				.filter_map(|(_, _, slot)| slot.upgrade())
				.collect()
		})
		.unwrap_or_default()
}

/// The handler Return or Escape runs in `window`, if any.
fn key_handler(key: &str, window: &Window, cx: &App) -> Option<KeyHandler> {
	let role = match key {
		"escape" => KeyRole::Dismiss,
		"enter" => {
			// A focused control that is not a passive surface consumes Return itself.
			if let Some(focused) = window.focused(cx) {
				let passes = live_slots(KeyRole::PassReturn, window, cx)
					.into_iter()
					.chain(live_slots(KeyRole::Dismiss, window, cx))
					.any(|slot| slot.read(cx).focus.as_ref() == Some(&focused));
				if !passes {
					return None;
				}
			}
			KeyRole::DefaultAction
		}
		_ => return None,
	};
	live_slots(role, window, cx)
		.into_iter()
		.rev()
		.find_map(|slot| slot.read(cx).handler.clone())
}

/// How many tab stops [`focus_within`] steps through before it gives up.
const TAB_SCAN_LIMIT: usize = 256;

/// Moves focus one tab stop forward or back inside `scope`, wrapping at its ends: Tab from the
/// last control lands on the first rather than on the page behind a modal.
pub(crate) fn focus_within(scope: &FocusHandle, forward: bool, window: &mut Window, cx: &mut App) {
	for _ in 0..TAB_SCAN_LIMIT {
		if forward {
			window.focus_next(cx);
		} else {
			window.focus_prev(cx);
		}
		if scope.contains_focused(window, cx) {
			return;
		}
	}
	// Nothing inside takes focus: park it on the scope itself.
	window.focus(scope, cx);
}

/// Tab or Shift-Tab: one tab stop on, confined to the topmost modal when one is up. Text
/// fields that handle Tab themselves should call this instead of `Window::focus_next`.
pub fn tab_step(forward: bool, window: &mut Window, cx: &mut App) {
	let scope = live_slots(KeyRole::TabScope, window, cx)
		.into_iter()
		.rev()
		.find_map(|slot| slot.read(cx).focus.clone());
	match scope {
		Some(scope) => focus_within(&scope, forward, window, cx),
		None if forward => window.focus_next(cx),
		None => window.focus_prev(cx),
	}
}

/// Binds Tab and Shift-Tab to move focus, and routes Return to the default action and
/// Escape to the topmost menu or modal. Call it once; later calls do nothing.
pub fn init_keyboard(cx: &mut App) {
	if cx.has_global::<KeyboardInstalled>() {
		return;
	}
	cx.set_global(KeyboardInstalled);
	// Unscoped bindings lose to a deeper key context, so a text editor keeps its Tab.
	cx.bind_keys([
		KeyBinding::new("tab", FocusNext, None),
		KeyBinding::new("shift-tab", FocusPrevious, None),
	]);
	// Global action listeners get no window; move focus once the dispatch returns it.
	cx.on_action(|_: &FocusNext, cx| {
		if let Some(window) = cx.active_window() {
			cx.defer(move |cx| {
				window.update(cx, |_, window, cx| tab_step(true, window, cx)).ok();
			});
		}
	});
	cx.on_action(|_: &FocusPrevious, cx| {
		if let Some(window) = cx.active_window() {
			cx.defer(move |cx| {
				window
					.update(cx, |_, window, cx| tab_step(false, window, cx))
					.ok();
			});
		}
	});
	// An interceptor sees the window and runs before key bindings, so Return and Escape
	// work even when no control has focus. Escape in a text input is the input's own: it
	// ends a composition and blurs the field before it can dismiss anything.
	cx.intercept_keystrokes(|event, window, cx| {
		let stroke = &event.keystroke;
		if stroke.modifiers.modified() {
			return;
		}
		if stroke.key == "escape" && text_input_focused(window) {
			return;
		}
		if let Some(handler) = key_handler(&stroke.key, window, cx) {
			cx.stop_propagation();
			handler(window, cx);
		}
	})
	.detach();
}

/// Whether a text input has focus. It owns Escape (ending a composition, then blurring) and
/// Space, so neither dismisses a surface nor presses a control.
pub(crate) fn text_input_focused(window: &Window) -> bool {
	window
		.context_stack()
		.iter()
		.any(|context| context.contains(crate::keymap::INPUT_CONTEXT))
}

/// The step that arrow keys move a value: `step`, or 5 % of the range.
pub(crate) fn key_step(range: &std::ops::RangeInclusive<f32>, step: Option<f32>) -> f32 {
	step.filter(|step| *step > 0.)
		.unwrap_or_else(|| (range.end() - range.start()).abs() * 0.05)
}

/// The direction of an unmodified arrow key: -1 for left and down, 1 for right and up.
pub(crate) fn arrow_direction(event: &KeyDownEvent) -> Option<i32> {
	if event.keystroke.modifiers.modified() {
		return None;
	}
	match event.keystroke.key.as_str() {
		"left" | "down" => Some(-1),
		"right" | "up" => Some(1),
		_ => None,
	}
}
