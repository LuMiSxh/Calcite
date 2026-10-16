// SPDX-License-Identifier: MPL-2.0

//! A trigger whose own glass morphs into a larger surface attached to its edge.
//!
//! The trigger is a real element that never moves, is never replaced, and never changes size. Behind
//! it sits ONE glass surface whose rect and corner radius follow the spring `t`: at `t = 0` it is
//! exactly the trigger's rect and radius (a capsule for a round trigger) in the trigger's own look,
//! at `t = 1` it is the trigger plus the attached body, in the body's look. Fill, edge, and material
//! are a plain time mix ([`Look::mix`]) of the two looks. Opening uses [`Curve::Present`]; closing
//! uses [`MORPH_CLOSE`], which rebounds, and [`motion::spring`](fn@motion::spring) keeps the velocity when the target flips.
//!
//! - While the surface shows, the trigger paints no border and no glass of its own
//!   ([`TriggerState::rim`] is 0) and only its content stays, drawn above the surface at every
//!   frame. At rest the trigger paints its own look, which the surface matches at `t = 0`
//!   exactly, so nothing jumps in either direction. The surface leaves the tree when the spring
//!   has settled at 0 and stopped, at which point it is the trigger's rect and radius.
//! - Every edge of the surface moves from the trigger's edge to its final place, and none ever
//!   goes more than [`MAX_OVERSHOOT_PX`] past its final place.
//! - Closing rebounds outward: the surface is never smaller than the trigger. While `t < 0` it
//!   swells uniformly past the trigger by at most [`CLOSE_BUMP_PX`] per edge (the radius grows with
//!   it, so a capsule stays a capsule) and then settles back. The surface stays the only glass and
//!   covers the trigger's rect, with the trigger's content painted above it, so nothing is doubled
//!   and nothing switches at `t = 0`; it ends exactly on the trigger's rect and radius.
//! - The direction (down, up, or flipped by the final size and the window) is decided once, when
//!   the body opens, never while it moves.
//! - The body's content fades and slides in during the second half of `t` and out during the
//!   first half.
//! - Reduce Transparency: plain fills and a crossfade instead of glass. Reduce Motion: the spring
//!   is instant.
//!
//! ```ignore
//! attached_surface("legend")
//!     .open(self.open)
//!     .trigger(|state, _, _| my_button(state.rim).into_any_element())
//!     .body((260., 300.), 20., |_, _| legend_rows().into_any_element())
//!     .align(Align::End)
//! ```
//!
//! The geometry, the gradient, and the flip logic are pure functions ([`surface_frame`],
//! [`trigger_rim`], [`resolve_direction`], [`body_shift`], [`Look::mix`]) so the endpoints can be tested.
//!
//! # Why placement is not gpui's `anchored()`
//!
//! `anchored()` places one child at a point and snaps it into the window, but it hides the rect it
//! chose and knows nothing of the trigger. The morph needs both: ONE surface interpolated from the
//! trigger's rect to the union of trigger and body, with the content laid out at the body's final
//! rect on every frame. So the window fit is computed here, from the trigger's measured bounds,
//! and the result stays visible to the geometry:
//!
//! - the direction flips Up/Down when the body does not fit ([`resolve_direction`]);
//! - the body size is capped to the window minus the margin on every side, so a window shorter than
//!   the body still shows the body inside it;
//! - the body is shifted horizontally and vertically to stay at least `margin` (8 px by default)
//!   inside the window ([`body_shift`]). The shift moves only the body's final rect; the surface still
//!   starts exactly on the trigger and the edges still travel from the trigger to their final place.

use std::rc::Rc;

use gpui::{
	AnyElement, App, Bounds, BoxShadow, ElementId, Filter, IntoElement, ParentElement, Pixels, Rgba, Role,
	Styled, Window, canvas, deferred, div, point, prelude::*, px,
};

use crate::{
	glass::{Elevation, Glass},
	morph::{self, Direction},
	motion::{self, secondary},
	theme::{ActiveTheme, Theme},
};

mod geometry;
mod surface;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod window_tests;

pub use geometry::{Align, Placement, body_shift, final_width, resolve_direction};
pub use surface::{
	Look, Surface, body_present, body_spring, surface_frame, surface_frame_shifted, trigger_rim,
};

/// How far the body's content slides while it fades.
pub const CONTENT_SLIDE: f32 = 6.;
/// The most an edge of the surface may go past its final place, in px, however large the surface
/// is: a ratio would let a 230 px card swing 6 px around a 28 px trigger.
pub const MAX_OVERSHOOT_PX: f32 = 2.;
/// The most an edge of the surface may swell past the trigger while closing rebounds, in px.
pub const CLOSE_BUMP_PX: f32 = 4.;

#[cfg(test)]
thread_local! {
	/// The inner (padding box) bounds of the surface the last frame painted, for the window tests.
	static LAST_SURFACE: std::cell::RefCell<Option<Bounds<Pixels>>> = const { std::cell::RefCell::new(None) };
}

/// What the trigger builder gets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerState {
	/// The raw spring value.
	pub t: f32,
	/// The clamped progress, 0 to 1.
	pub unit: f32,
	/// Whether the body is meant to be open (the target, not the animated state).
	pub open: bool,
	/// Opacity of the trigger's own glass and border; see [`trigger_rim`]. Apply it to the
	/// surface and the border only, never to the content.
	pub rim: f32,
}

type TriggerBuilder = Box<dyn FnOnce(TriggerState, &mut Window, &mut App) -> AnyElement>;
type BodyBuilder = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>;
type Dismiss = Rc<dyn Fn(&mut Window, &mut App)>;

struct BodySpec {
	size: (f32, f32),
	radius: f32,
	content: BodyBuilder,
}

/// A trigger plus an attached body. See the [module docs](self).
#[derive(IntoElement)]
pub struct AttachedSurface {
	id: ElementId,
	open: bool,
	trigger: Option<TriggerBuilder>,
	body: Option<BodySpec>,
	trigger_glass: Option<(Glass, Rgba)>,
	body_glass: Option<(Glass, Rgba)>,
	trigger_radius: Option<f32>,
	placement: Placement,
	align: Align,
	offset: f32,
	margin: f32,
	role: Option<Role>,
	dismiss: Option<Dismiss>,
	theme: Option<Theme>,
}

/// An attached surface with the given element id. It holds the spring and the trigger probe in
/// keyed state under that id.
pub fn attached_surface(id: impl Into<ElementId>) -> AttachedSurface {
	AttachedSurface {
		id: id.into(),
		open: false,
		trigger: None,
		body: None,
		trigger_glass: None,
		body_glass: None,
		trigger_radius: None,
		placement: Placement::Down,
		align: Align::Start,
		offset: 0.,
		margin: 8.,
		role: None,
		dismiss: None,
		theme: None,
	}
}

impl AttachedSurface {
	/// Whether the body should be open.
	pub fn open(mut self, open: bool) -> Self {
		self.open = open;
		self
	}

	/// Builds the trigger. It must paint its own glass and border scaled by
	/// [`TriggerState::rim`], and keep its layout the same at every `rim`.
	pub fn trigger(
		mut self,
		build: impl FnOnce(TriggerState, &mut Window, &mut App) -> AnyElement + 'static,
	) -> Self {
		self.trigger = Some(Box::new(build));
		self
	}

	/// The body: its final size, its corner radius, and its content, which is only built while it
	/// shows and is clipped to the growing surface.
	pub fn body(
		mut self,
		size: (f32, f32),
		radius: f32,
		content: impl FnOnce(&mut Window, &mut App) -> AnyElement + 'static,
	) -> Self {
		self.body = Some(BodySpec {
			size,
			radius,
			content: Box::new(content),
		});
		self
	}

	/// The glass and outline of the trigger at rest. The default is the resting HUD surface.
	pub fn trigger_look(mut self, glass: Glass, edge: Rgba) -> Self {
		self.trigger_glass = Some((glass, edge));
		self
	}

	/// The glass and outline of the open body. The default is the raised HUD surface.
	pub fn body_look(mut self, glass: Glass, edge: Rgba) -> Self {
		self.body_glass = Some((glass, edge));
		self
	}

	/// The trigger's corner radius in px, for the fused shape. Default: a capsule.
	pub fn trigger_radius(mut self, radius: f32) -> Self {
		self.trigger_radius = Some(radius);
		self
	}

	pub fn placement(mut self, placement: Placement) -> Self {
		self.placement = placement;
		self
	}

	pub fn align(mut self, align: Align) -> Self {
		self.align = align;
		self
	}

	/// Distance between the trigger's edge and the body's: positive leaves a gap, negative
	/// overlaps the trigger. Default 0, flush.
	pub fn offset(mut self, offset: f32) -> Self {
		self.offset = offset;
		self
	}

	/// Window margin that [`Placement::Auto`] keeps free.
	pub fn margin(mut self, margin: f32) -> Self {
		self.margin = margin;
		self
	}

	/// The accessibility role of the body.
	pub fn body_role(mut self, role: Role) -> Self {
		self.role = Some(role);
		self
	}

	/// Called on a mouse press outside the body and outside the trigger.
	pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
		self.dismiss = Some(Rc::new(handler));
		self
	}

	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

fn scaled_shadows(shadows: Vec<BoxShadow>, strength: f32) -> Vec<BoxShadow> {
	shadows
		.into_iter()
		.map(|shadow| BoxShadow {
			color: shadow.color.opacity(strength),
			offset: point(shadow.offset.x, shadow.offset.y * strength),
			blur_radius: shadow.blur_radius * strength,
			..shadow
		})
		.collect()
}

impl RenderOnce for AttachedSurface {
	fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.take().unwrap_or_else(|| cx.theme());
		let id = self.id.clone();
		let sample = motion::spring(
			(id.clone(), "calcite-attached"),
			if self.open { 1. } else { 0. },
			body_spring(self.open),
			theme.reduce_motion,
			window,
			cx,
		);
		let t = sample.value;
		let probe = window.use_keyed_state((id.clone(), "calcite-attached-trigger"), cx, |_, _| {
			None::<Bounds<Pixels>>
		});
		let chosen = window.use_keyed_state((id.clone(), "calcite-attached-direction"), cx, |_, _| {
			None::<Direction>
		});
		let trigger_bounds = *probe.read(cx);
		let present = body_present(t, self.open, sample.active);
		if !present && chosen.read(cx).is_some() {
			chosen.update(cx, |direction, _| *direction = None);
		}
		let visible = present && self.body.is_some() && trigger_bounds.is_some();

		let (trigger_glass, trigger_edge) = self.trigger_glass.take().unwrap_or_else(|| {
			(
				theme.hud_surface().elevation(Elevation::Resting),
				theme.hud_border(),
			)
		});
		let (body_glass, body_edge) = self.body_glass.take().unwrap_or_else(|| {
			(
				theme.hud_surface().elevation(Elevation::Raised),
				theme.hud_border(),
			)
		});
		let from = Look::of(&trigger_glass, trigger_edge);
		let to = Look::of(&body_glass, body_edge);
		let glass = from.filter.is_some() && to.filter.is_some();
		let unit = t.clamp(0., 1.);
		let fused = visible && glass;

		let rim = if visible { trigger_rim(t, glass) } else { 1. };
		let state = TriggerState {
			t,
			unit,
			open: self.open,
			rim,
		};
		let trigger = self
			.trigger
			.take()
			.map(|build| build(state, window, cx))
			.unwrap_or_else(|| div().into_any_element());
		// Shrink-wrapped, so the reported trigger bounds are the trigger's even in a block parent.
		let trigger = div()
			.id((id.clone(), "trigger"))
			.relative()
			.flex()
			.child(trigger)
			.child(
				canvas(
					{
						let probe = probe.clone();
						move |bounds, _, cx| probe.update(cx, |stored, _| *stored = Some(bounds))
					},
					|_, _, _, _| {},
				)
				.absolute()
				.top_0()
				.left_0()
				.size_full(),
			);

		let group_id: ElementId = (id.clone(), "calcite-attached-root").into();
		let (Some(bounds), true) = (trigger_bounds, visible) else {
			#[cfg(test)]
			LAST_SURFACE.with(|last| *last.borrow_mut() = None);
			return div()
				.id(group_id)
				.relative()
				.flex()
				.flex_col()
				.items_start()
				.child(trigger)
				.into_any_element();
		};
		let mut spec = self.body.take().expect("visible implies a body");
		let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
		// The body is never narrower than the trigger it hangs from.
		spec.size.0 = spec.size.0.max(size.0);
		let viewport = window.viewport_size();
		let viewport = (f32::from(viewport.width), f32::from(viewport.height));
		let direction = match *chosen.read(cx) {
			Some(direction) => direction,
			None => {
				let direction = resolve_direction(
					self.placement,
					f32::from(bounds.origin.y),
					size.1,
					spec.size.1,
					self.offset,
					viewport.1,
					self.margin,
				);
				chosen.update(cx, |stored, _| *stored = Some(direction));
				direction
			}
		};
		let trigger_radius = self.trigger_radius.unwrap_or(size.0.min(size.1) / 2.);
		// Never taller or wider than the window allows, then moved inside it.
		let cap = |size: f32, limit: f32| size.min((limit - 2. * self.margin).max(0.));
		spec.size = (cap(spec.size.0, viewport.0), cap(spec.size.1, viewport.1));
		let shift = body_shift(
			direction,
			self.align,
			self.offset,
			(f32::from(bounds.origin.x), f32::from(bounds.origin.y)),
			size,
			(final_width(self.align, size.0, spec.size.0), spec.size.1),
			viewport,
			self.margin,
		);
		let frame = surface_frame_shifted(
			t,
			direction,
			self.align,
			self.offset,
			size,
			spec.size,
			trigger_radius,
			spec.radius,
			shift,
		);

		// Without glass (Reduce Transparency) the surface only crossfades in.
		let shown = fused || (visible && !glass);
		let surface = shown.then(|| {
			let mut look = from.mix(&to, frame.unit);
			// Fast glass catches more light and bends the backdrop harder.
			let boost = secondary::glass_boost(secondary::speed_factor(sample.velocity), theme.reduce_motion);
			look.filter = look
				.filter
				.take()
				.map(|filter| morph::boost_filter(filter, boost));
			let content = (frame.content > 0.).then(|| {
				let content = (spec.content)(window, cx);
				// Inside the border box, so the 1 px border shifts absolute children.
				let left = frame.body_left - frame.left - 1.;
				let top = frame.body_top - frame.top - 1.;
				div()
					.absolute()
					.left(px(left))
					.top(px(match direction {
						Direction::Down => top - frame.slide,
						Direction::Up => top + frame.slide,
					}))
					.w(px(frame.body_width))
					.opacity(frame.content)
					.child(content)
			});
			let dismiss = self.dismiss.clone();
			let mut surface = div()
				.id((id.clone(), "calcite-attached-body"))
				.absolute()
				.left(px(frame.left))
				.top(px(frame.top))
				.w(px(frame.width))
				.h(px(frame.height))
				.overflow_hidden()
				.rounded(px(frame.radius))
				.border_1()
				.border_color(look.edge)
				.shadow(scaled_shadows(body_glass.shadows(), frame.shadow))
				.occlude()
				.when_some(self.role, |surface, role| surface.role(role))
				.when_some(dismiss, |surface, dismiss| {
					surface.on_mouse_down_out(move |event, window, cx| {
						if !bounds.contains(&event.position) {
							dismiss(window, cx);
						}
					})
				});
			surface = match look.filter {
				Some(filter) if fused => surface.backdrop_filter(vec![Filter::Glass(filter)]),
				_ => surface.bg(look.fill).opacity(frame.unit),
			};
			#[cfg(test)]
			{
				surface = surface.child(
					canvas(
						|bounds, _, _| LAST_SURFACE.with(|last| *last.borrow_mut() = Some(bounds)),
						|_, _, _, _| {},
					)
					.absolute()
					.top_0()
					.left_0()
					.size_full(),
				);
			}
			surface.children(content)
		});
		#[cfg(test)]
		if surface.is_none() {
			LAST_SURFACE.with(|last| *last.borrow_mut() = None);
		}

		// The surface comes first, so the trigger's content paints above it every frame.
		let root = div()
			.id(group_id)
			.relative()
			.flex()
			.flex_col()
			.items_start()
			.children(surface)
			.child(trigger)
			.into_any_element();
		// Lifted above its siblings so the body overlays what follows the trigger in the layout.
		deferred(root).priority(1).into_any_element()
	}
}
