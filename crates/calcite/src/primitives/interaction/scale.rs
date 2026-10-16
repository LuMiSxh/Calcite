// SPDX-License-Identifier: MPL-2.0

//! Scale about an anchor: a paint transform, so layout never changes.

use gpui::{
	AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
	Pixels, Point, Window, point,
};

use crate::motion::{
	Spring,
	tokens::{JELLY, PRESS},
};

/// The spring behind a control's scale: [`PRESS`] while held down, [`JELLY`] on release (and when
/// the hover lift ends), so letting go springs back with a visible wobble.
pub fn scale_spring(pressed: bool) -> Spring {
	if pressed { PRESS } else { JELLY }
}

impl ScaleAnchor {
	pub(super) fn origin(self, bounds: Bounds<Pixels>) -> Point<Pixels> {
		match self {
			Self::Center => bounds.center(),
			Self::BottomCenter => point(bounds.center().x, bounds.bottom()),
		}
	}
}

/// Whether a scale is close enough to 1 to skip the paint transform.
pub(super) fn is_unscaled(scale: f32) -> bool {
	(scale - 1.).abs() < 1e-4
}

/// Where `p` lands when scaled by `scale` about `origin`.
#[cfg(test)]
pub(super) fn scale_about(p: Point<Pixels>, origin: Point<Pixels>, scale: f32) -> Point<Pixels> {
	origin + (p - origin) * scale
}

/// The point that lands on `p` once scaled by `scale` about `origin`.
pub(super) fn unscale_about(p: Point<Pixels>, origin: Point<Pixels>, scale: f32) -> Point<Pixels> {
	if scale.abs() < 1e-6 {
		return p;
	}
	origin + (p - origin) / scale
}

impl ScaleEffect {
	pub fn anchor(mut self, anchor: ScaleAnchor) -> Self {
		self.anchor = anchor;
		self
	}
}

/// Scales everything `child` paints by `scale` about its center (SwiftUI `scaleEffect`).
pub fn scale_effect(scale: f32, child: impl IntoElement) -> ScaleEffect {
	ScaleEffect {
		scale,
		anchor: ScaleAnchor::Center,
		child: child.into_any_element(),
	}
}

/// `element.scaled(frame.scale)`: the whole control scales, layout and hitboxes do not.
pub trait ScaleExt: IntoElement + Sized {
	fn scaled(self, scale: f32) -> ScaleEffect {
		scale_effect(scale, self)
	}
}
impl<E: IntoElement> ScaleExt for E {}

/// Where a [`ScaleEffect`] keeps its fixed point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScaleAnchor {
	#[default]
	Center,
	/// The middle of the bottom edge, where a HUD pill rises from the control bar.
	BottomCenter,
}

impl IntoElement for ScaleEffect {
	type Element = Self;

	fn into_element(self) -> Self::Element {
		self
	}
}

impl Element for ScaleEffect {
	type RequestLayoutState = ();
	type PrepaintState = ();

	fn id(&self) -> Option<ElementId> {
		None
	}

	fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
		None
	}

	fn request_layout(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		window: &mut Window,
		cx: &mut App,
	) -> (LayoutId, Self::RequestLayoutState) {
		(self.child.request_layout(window, cx), ())
	}

	fn prepaint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		_bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		window: &mut Window,
		cx: &mut App,
	) -> Self::PrepaintState {
		self.child.prepaint(window, cx);
	}

	fn paint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		_prepaint: &mut Self::PrepaintState,
		window: &mut Window,
		cx: &mut App,
	) {
		if is_unscaled(self.scale) {
			self.child.paint(window, cx);
		} else {
			let origin = self.anchor.origin(bounds);
			let child = &mut self.child;
			window.with_paint_transform(self.scale, origin, |window| child.paint(window, cx));
		}
	}
}

/// Paints its child scaled about an anchor without touching layout or hit-testing.
pub struct ScaleEffect {
	scale: f32,
	anchor: ScaleAnchor,
	child: AnyElement,
}
