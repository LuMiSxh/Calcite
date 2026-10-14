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
//! - Closing rebounds: while `t < 0` the whole surface shrinks uniformly inside the trigger by at
//!   most [`CLOSE_DIP_PX`] per edge (radius shrinks with it, so a capsule stays a capsule). The
//!   trigger paints no glass of its own then either, so the dip is just the glass getting smaller
//!   with nothing doubled and nothing to switch at `t = 0`; the surface still ends exactly on the
//!   trigger's rect and radius.
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
//! [`trigger_rim`], [`resolve_direction`], [`Look::mix`]) so the endpoints can be tested.

use std::rc::Rc;

use gpui::{
	AnyElement, App, Bounds, BoxShadow, ElementId, Filter, GlassFilter, IntoElement, ParentElement, Pixels,
	Rgba, Role, Styled, Window, canvas, deferred, div, point, prelude::*, px,
};

use crate::{
	glass::{Elevation, Glass},
	morph::{self, Direction, MAX_OVERSHOOT},
	motion::{self, Curve, Spring, secondary, tokens::MORPH_CLOSE},
	theme::{ActiveTheme, Theme},
};

/// How far the body's content slides while it fades.
pub const CONTENT_SLIDE: f32 = 6.;
/// The most an edge of the surface may go past its final place, in px, however large the surface
/// is: a ratio would let a 230 px card swing 6 px around a 28 px trigger.
pub const MAX_OVERSHOOT_PX: f32 = 2.;
/// The most an edge of the surface may dip inside the trigger while closing rebounds, in px.
pub const CLOSE_DIP_PX: f32 = 4.;

#[cfg(test)]
thread_local! {
	/// The inner (padding box) bounds of the surface the last frame painted, for the window tests.
	static LAST_SURFACE: std::cell::RefCell<Option<Bounds<Pixels>>> = const { std::cell::RefCell::new(None) };
}

/// Which side of the trigger the body grows toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
	Down,
	Up,
	/// Down, unless the final body would leave the window and up fits better.
	Auto,
}

/// How the body lines up with the trigger along the trigger's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
	/// Left edges aligned.
	Start,
	Center,
	/// Right edges aligned.
	End,
	/// The body is exactly as wide as the trigger.
	Stretch,
}

/// The look of one end of the morph: material, flat fill, and outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
	/// `None` under Reduce Transparency.
	pub filter: Option<GlassFilter>,
	pub fill: Rgba,
	pub edge: Rgba,
}

impl Look {
	pub fn of(glass: &Glass, edge: Rgba) -> Self {
		Self {
			filter: glass.filter(),
			fill: glass.fill(),
			edge,
		}
	}

	/// The look between `self` (at `t = 0`) and `to` (at `t = 1`).
	pub fn mix(&self, to: &Look, t: f32) -> Look {
		Look {
			filter: match (&self.filter, &to.filter) {
				(Some(from), Some(to)) => Some(morph::mix_filter(from, to, t)),
				_ => None,
			},
			fill: morph::gradient(self.fill, to.fill, t),
			edge: morph::gradient(self.edge, to.edge, t),
		}
	}
}

/// The surface at one instant, relative to the trigger's top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
	pub left: f32,
	pub top: f32,
	pub width: f32,
	pub height: f32,
	pub radius: f32,
	/// The body's final rect (where its content lays out), relative to the trigger.
	pub body_left: f32,
	pub body_top: f32,
	pub body_width: f32,
	/// Opacity of the body's content: 0 in the first half of `t`, rising to 1 in the second.
	pub content: f32,
	/// How far the content is still displaced, in px.
	pub slide: f32,
	/// Strength of the surface's drop shadow, 0 at rest.
	pub shadow: f32,
	/// The clamped progress, for the colour gradient.
	pub unit: f32,
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
	from + (to - from) * t
}

/// The largest spring value that still moves an edge `span` px: 1 plus the overshoot
/// allowance, which is [`MAX_OVERSHOOT_PX`] but never more than the ratio [`MAX_OVERSHOOT`].
fn max_grow(span: f32) -> f32 {
	if span <= 0. {
		return 1.;
	}
	1. + (MAX_OVERSHOOT_PX / span).min(MAX_OVERSHOOT - 1.)
}

/// The spring that drives the body: [`Curve::Present`] while opening, [`MORPH_CLOSE`] while
/// closing. [`motion::spring`](fn@motion::spring) keeps the velocity when the target flips.
pub fn body_spring(open: bool) -> Spring {
	if open {
		Curve::Present.spring().expect("Present is a spring")
	} else {
		MORPH_CLOSE
	}
}

/// How far the surface sits inside the trigger when the spring is `depth` below 0: the pull
/// (`depth` times the longest edge travel) saturates smoothly at [`CLOSE_DIP_PX`], so a large
/// body rebounds visibly and a small one proportionally, and never more than a quarter of the
/// trigger's short side (the size stays positive).
fn close_dip(depth: f32, span: f32, trigger: (f32, f32)) -> f32 {
	let cap = CLOSE_DIP_PX.min(trigger.0.min(trigger.1) / 4.).max(0.);
	if cap <= 0. || span <= 0. || depth <= 0. {
		return 0.;
	}
	cap * (depth * span / cap).tanh()
}

/// The width the body ends at: the trigger's for [`Align::Stretch`], otherwise its own.
pub fn final_width(align: Align, trigger_width: f32, body_width: f32) -> f32 {
	match align {
		Align::Stretch => trigger_width,
		_ => body_width,
	}
}

/// The surface at spring value `t`. At 0 it is exactly the trigger's rect, with the trigger's
/// radius; at 1 it covers the trigger and `body` (the trigger's width when stretched), the body
/// placed `offset` away from the trigger's edge (positive is a gap, negative overlaps the
/// trigger). Overshoot moves an edge at most [`MAX_OVERSHOOT_PX`] past its final place;
/// undershoot (`t < 0`) shrinks the rect uniformly inside the trigger by at most
/// [`CLOSE_DIP_PX`] per edge, with the radius shrinking along.
#[allow(clippy::too_many_arguments)]
pub fn surface_frame(
	t: f32,
	direction: Direction,
	align: Align,
	offset: f32,
	trigger: (f32, f32),
	body: (f32, f32),
	trigger_radius: f32,
	body_radius: f32,
) -> Surface {
	let unit = t.clamp(0., 1.);
	let (tw, th) = trigger;
	let bw = final_width(align, tw, body.0);
	let bh = body.1.max(0.);
	let body_left = match align {
		Align::Start | Align::Stretch => 0.,
		Align::Center => (tw - bw) / 2.,
		Align::End => tw - bw,
	};
	let body_top = match direction {
		Direction::Down => th + offset,
		Direction::Up => -offset - bh,
	};
	// The final surface is the trigger and the body together. Each edge travels from the
	// trigger's edge to its final place; an edge that does not move never moves.
	let edge = |from: f32, to: f32| lerp(from, to, t.clamp(0., max_grow((to - from).abs())));
	let (to_left, to_top) = (body_left.min(0.), body_top.min(0.));
	let (to_right, to_bottom) = ((body_left + bw).max(tw), (body_top + bh).max(th));
	let dip = if t < 0. {
		let span = [-to_left, -to_top, to_right - tw, to_bottom - th]
			.into_iter()
			.fold(0., f32::max);
		close_dip(-t, span, trigger)
	} else {
		0.
	};
	let left = edge(0., to_left) + dip;
	let top = edge(0., to_top) + dip;
	let right = edge(tw, to_right) - dip;
	let bottom = edge(th, to_bottom) - dip;
	let (width, height) = (right - left, bottom - top);
	let content = ((unit - 0.5) / 0.5).clamp(0., 1.);
	Surface {
		left,
		top,
		width,
		height,
		radius: (lerp(trigger_radius, body_radius, unit) - dip)
			.min(width.min(height) / 2.)
			.max(0.),
		body_left,
		body_top,
		body_width: bw,
		content,
		slide: (1. - content) * CONTENT_SLIDE,
		shadow: unit,
		unit,
	}
}

/// How much of its own glass and border the trigger paints while the surface exists (when it does
/// not, the trigger always paints it all). With glass the surface owns the look from its first to
/// its last frame, so this is 0 throughout: the two are never drawn together, and the frame the
/// surface leaves the trigger takes over a look identical to it. Without glass (Reduce
/// Transparency) it crossfades with the surface.
pub fn trigger_rim(t: f32, glass: bool) -> f32 {
	if glass { 0. } else { 1. - t.clamp(0., 1.) }
}

/// Whether the body is still part of the tree: until the spring has settled closed.
/// `active` is the spring's own verdict (still moving), which also keeps the body for the very
/// last frames, so it leaves exactly when the spring snaps onto 0 with no velocity.
pub fn body_present(t: f32, open: bool, active: bool) -> bool {
	open || active || t > 0.
}

/// Which way the body grows, from its final height and the window. Call it once per opening.
/// `Auto` prefers down; if the body does not fit below, it goes up when that fits or has more room.
pub fn resolve_direction(
	placement: Placement,
	trigger_top: f32,
	trigger_height: f32,
	body_height: f32,
	offset: f32,
	viewport_height: f32,
	margin: f32,
) -> Direction {
	match placement {
		Placement::Down => Direction::Down,
		Placement::Up => Direction::Up,
		Placement::Auto => {
			let below_end = trigger_top + trigger_height + offset + body_height;
			if below_end + margin <= viewport_height {
				return Direction::Down;
			}
			let above_start = trigger_top - offset - body_height;
			if above_start >= margin {
				return Direction::Up;
			}
			let room_below = viewport_height - (trigger_top + trigger_height);
			if trigger_top > room_below {
				Direction::Up
			} else {
				Direction::Down
			}
		}
	}
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
		let direction = match *chosen.read(cx) {
			Some(direction) => direction,
			None => {
				let direction = resolve_direction(
					self.placement,
					f32::from(bounds.origin.y),
					size.1,
					spec.size.1,
					self.offset,
					f32::from(window.viewport_size().height),
					self.margin,
				);
				chosen.update(cx, |stored, _| *stored = Some(direction));
				direction
			}
		};
		let trigger_radius = self.trigger_radius.unwrap_or(size.0.min(size.1) / 2.);
		let frame = surface_frame(
			t,
			direction,
			self.align,
			self.offset,
			size,
			spec.size,
			trigger_radius,
			spec.radius,
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tokens::{black, white};

	const TRIGGER: (f32, f32) = (80., 24.);
	const BODY: (f32, f32) = (180., 120.);

	fn frame_at(t: f32, direction: Direction, align: Align, offset: f32) -> Surface {
		surface_frame(t, direction, align, offset, TRIGGER, BODY, 12., 16.)
	}

	#[test]
	fn endpoints_are_exact() {
		let closed = frame_at(0., Direction::Down, Align::Start, 0.);
		assert_eq!((closed.left, closed.top), (0., 0.));
		assert_eq!((closed.width, closed.height, closed.radius), (80., 24., 12.));
		assert_eq!((closed.content, closed.shadow, closed.unit), (0., 0., 0.));
		let open = frame_at(1., Direction::Down, Align::Start, 0.);
		assert_eq!((open.width, open.height, open.radius), (180., 144., 16.));
		assert_eq!((open.content, open.shadow, open.slide), (1., 1., 0.));
		let half = frame_at(0.5, Direction::Down, Align::Start, 0.);
		assert!((half.radius - 14.).abs() < 1e-5);
	}

	#[test]
	fn at_rest_the_surface_is_exactly_the_trigger_for_every_placement() {
		for direction in [Direction::Down, Direction::Up] {
			for align in [Align::Start, Align::Center, Align::End, Align::Stretch] {
				for offset in [-24., 0., 4.] {
					let s = frame_at(0., direction, align, offset);
					assert_eq!(
						(s.left, s.top, s.width, s.height, s.radius),
						(0., 0., TRIGGER.0, TRIGGER.1, 12.),
						"{direction:?} {align:?} offset {offset}"
					);
					// A hair below 0 is a hair inside the trigger: continuous at the crossing.
					let s = frame_at(-1e-6, direction, align, offset);
					assert!(s.left < 1e-3 && s.top < 1e-3 && (s.width - TRIGGER.0).abs() < 1e-3);
					assert!((s.height - TRIGGER.1).abs() < 1e-3 && (s.radius - 12.).abs() < 1e-3);
				}
			}
		}
		// A capsule trigger stays a capsule: radius is half the height.
		let capsule = surface_frame(
			0.,
			Direction::Down,
			Align::End,
			-28.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		);
		assert_eq!((capsule.width, capsule.height, capsule.radius), (28., 28., 14.));
	}

	#[test]
	fn undershoot_dips_inside_the_trigger_by_at_most_the_cap_and_overshoot_is_capped() {
		for direction in [Direction::Down, Direction::Up] {
			for align in [Align::Start, Align::Center, Align::End, Align::Stretch] {
				let mut last_dip = 0.;
				for step in 1..=400 {
					let t = -(step as f32) * 0.005;
					let s = frame_at(t, direction, align, 0.);
					let dip = s.left;
					// Uniform on all four edges, inside the trigger, never past the cap.
					assert!((s.top - dip).abs() < 1e-4, "{direction:?} {align:?} t {t}");
					assert!((TRIGGER.0 - (s.left + s.width) - dip).abs() < 1e-4);
					assert!((TRIGGER.1 - (s.top + s.height) - dip).abs() < 1e-4);
					assert!(dip > 0. && dip <= CLOSE_DIP_PX + 1e-4, "dip {dip}");
					assert!(s.width > 0. && s.height > 0. && s.radius >= 0.);
					assert!(s.radius <= s.width.min(s.height) / 2. + 1e-4);
					// Deeper rebound, deeper dip.
					assert!(dip >= last_dip - 1e-5);
					last_dip = dip;
					assert_eq!((s.unit, s.shadow, s.content), (0., 0., 0.));
				}
				assert!(
					last_dip > CLOSE_DIP_PX - 0.1,
					"a deep rebound nearly reaches the cap"
				);
			}
		}
		// The radius shrinks with the rect: a capsule stays a capsule.
		let capsule = |t: f32| {
			surface_frame(
				t,
				Direction::Down,
				Align::End,
				-28.,
				(28., 28.),
				(260., 300.),
				14.,
				20.,
			)
		};
		for t in [-0.001, -0.02, -0.08, -0.5] {
			let s = capsule(t);
			assert!(
				(s.radius - s.width.min(s.height) / 2.).abs() < 1e-4,
				"t {t}: {s:?}"
			);
		}
		// A tiny trigger never collapses.
		let tiny = surface_frame(
			-1.,
			Direction::Down,
			Align::Start,
			0.,
			(6., 6.),
			(100., 100.),
			3.,
			8.,
		);
		assert!(tiny.width >= 3. - 1e-4 && tiny.height >= 3. - 1e-4);
		let s = frame_at(1.04, Direction::Down, Align::Start, 0.);
		assert!(s.width > 180. && s.height > 144.);
		let capped = frame_at(9., Direction::Down, Align::Start, 0.);
		// 2 px past the final edge, not a ratio of the growth.
		assert!((capped.height - 146.).abs() < 1e-3, "{}", capped.height);
		assert!((capped.width - 182.).abs() < 1e-3, "{}", capped.width);
		assert_eq!(capped.radius, 16.);
		// A big body overshoots by the same few px as a small one.
		let big = surface_frame(
			9.,
			Direction::Down,
			Align::End,
			0.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		);
		assert!((big.height - 330.).abs() < 1e-3 && (big.width - 262.).abs() < 1e-3);
		// A tiny body keeps the ratio cap of the morph module.
		let tiny = surface_frame(
			9.,
			Direction::Down,
			Align::Start,
			0.,
			(80., 24.),
			(80., 10.),
			12.,
			4.,
		);
		assert!((tiny.height - (24. + 10. * MAX_OVERSHOOT)).abs() < 1e-3);
	}

	#[test]
	fn radius_never_exceeds_half_the_short_side() {
		for step in 0..=120 {
			let s = surface_frame(
				step as f32 / 100.,
				Direction::Down,
				Align::Start,
				0.,
				TRIGGER,
				BODY,
				12.,
				80.,
			);
			assert!(s.radius <= s.width.min(s.height) / 2. + 1e-4);
		}
	}

	#[test]
	fn body_attaches_to_the_chosen_edge_with_its_offset() {
		let down = frame_at(1., Direction::Down, Align::Start, 2.);
		assert_eq!((down.body_top, down.top, down.height), (26., 0., 146.));
		let up = frame_at(1., Direction::Up, Align::Start, 2.);
		assert_eq!(up.body_top + 120., -2.);
		assert_eq!((up.top, up.top + up.height), (-122., 24.));
		let overlap = frame_at(1., Direction::Down, Align::Start, -24.);
		assert_eq!((overlap.top, overlap.body_top, overlap.height), (0., 0., 120.));
		// The edge on the trigger stays put while the surface grows.
		for t in [0.2, 0.6, 1.05] {
			let up = frame_at(t, Direction::Up, Align::Start, 1.);
			assert!((up.top + up.height - 24.).abs() < 1e-4);
			let down = frame_at(t, Direction::Down, Align::Start, 1.);
			assert_eq!(down.top, 0.);
		}
	}

	#[test]
	fn alignment_anchors_an_edge_while_growing() {
		for t in [0., 0.3, 1., 1.05] {
			let start = frame_at(t, Direction::Down, Align::Start, 0.);
			assert_eq!(start.left, 0.);
			let end = frame_at(t, Direction::Down, Align::End, 0.);
			assert!((end.left + end.width - TRIGGER.0).abs() < 1e-4);
			let center = frame_at(t, Direction::Down, Align::Center, 0.);
			assert!((center.left + center.width / 2. - TRIGGER.0 / 2.).abs() < 1e-4);
		}
		let stretch = frame_at(1., Direction::Down, Align::Stretch, 0.);
		assert_eq!((stretch.left, stretch.width), (0., 80.));
		assert_eq!(final_width(Align::Stretch, 80., 180.), 80.);
		assert_eq!(final_width(Align::End, 80., 180.), 180.);
	}

	#[test]
	fn content_arrives_in_the_second_half_and_leaves_in_the_first() {
		assert_eq!(frame_at(0.5, Direction::Down, Align::Start, 0.).content, 0.);
		assert_eq!(frame_at(0.3, Direction::Down, Align::Start, 0.).content, 0.);
		let late = frame_at(0.75, Direction::Down, Align::Start, 0.);
		assert!((late.content - 0.5).abs() < 1e-6);
		assert!(late.slide > 0.);
	}

	#[test]
	fn trigger_rim_never_overlaps_the_surface_with_glass() {
		// Including the dip: the surface alone is the glass, so crossing t = 0 switches nothing.
		for t in [-0.3, -0.02, -1e-6, 0., 0.0005, 0.001, 0.5, 1., 1.06] {
			assert_eq!(trigger_rim(t, true), 0., "t = {t}");
		}
	}

	#[test]
	fn the_dip_leaves_no_hole_in_the_glass() {
		// With glass, either the surface exists and the trigger paints no glass, or the reverse;
		// the visible glass shape is the surface, which is inside the trigger by <= the cap on
		// every edge, so the only uncovered pixels are the glass shrinking.
		let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
		let (frames, _) = run(open, false, 600);
		assert!(frames.iter().any(|(t, _)| *t < 0.), "the close never dipped");
		// The last frame is the settled one: exactly 0, where the surface is removed.
		let (settled, moving) = frames.split_last().expect("frames");
		assert!(!settled.1 && settled.0 == 0.);
		for (t, active) in moving {
			let present = body_present(*t, false, *active);
			assert!(present, "surface dropped at t = {t} while still moving");
			assert_eq!(trigger_rim(*t, true), 0.);
			let s = surface_frame(
				*t,
				Direction::Down,
				Align::End,
				-28.,
				(28., 28.),
				(260., 300.),
				14.,
				20.,
			);
			assert!(s.left <= CLOSE_DIP_PX + 1e-4 && s.top <= CLOSE_DIP_PX + 1e-4);
			assert!(28. - (s.left + s.width) <= CLOSE_DIP_PX + 1e-4);
			assert!(28. - (s.top + s.height) <= CLOSE_DIP_PX + 1e-4);
		}
		// The dip is where it is visible: several px, not a rounding error.
		let deepest = frames
			.iter()
			.map(|(t, _)| {
				surface_frame(
					*t,
					Direction::Down,
					Align::End,
					-28.,
					(28., 28.),
					(260., 300.),
					14.,
					20.,
				)
				.left
			})
			.fold(0., f32::max);
		assert!(deepest > 2., "dip only {deepest} px");
	}

	#[test]
	fn the_close_spring_rebounds_further_than_the_calm_one() {
		assert!(MORPH_CLOSE.overshoot() > 0.07);
	}

	#[test]
	fn the_handover_at_the_end_of_a_close_has_no_gap_or_overlap() {
		let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
		let (frames, _) = run(open, false, 600);
		// Each rendered frame: the surface exists exactly while the body is present, and the
		// trigger's rim is 0 exactly then, 1 otherwise.
		let coverage = |t: f32, active: bool| {
			let present = body_present(t, false, active);
			let rim = if present { trigger_rim(t, true) } else { 1. };
			(present as u8 as f32) + rim
		};
		for (t, active) in &frames {
			assert_eq!(coverage(*t, *active), 1., "t = {t}");
		}
		// The frame after the last animated one: the surface is gone, the trigger paints all.
		assert_eq!(coverage(0., false), 1.);
		// And the surface that is removed is the trigger's exact rect and radius.
		let last = surface_frame(
			frames.last().unwrap().0,
			Direction::Down,
			Align::End,
			-28.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		);
		assert_eq!((last.width, last.height, last.radius), (28., 28., 14.));
	}

	#[test]
	fn trigger_rim_crossfades_without_glass() {
		assert_eq!(trigger_rim(0., false), 1.);
		assert_eq!(trigger_rim(1., false), 0.);
		assert_eq!(trigger_rim(1.06, false), 0.);
		assert!((trigger_rim(0.25, false) - 0.75).abs() < 1e-6);
	}

	#[test]
	fn the_body_stays_until_settled_closed() {
		// Gone only when closed, at 0 and no longer moving.
		assert!(!body_present(0., false, false));
		assert!(body_present(0.3, false, true));
		assert!(body_present(0.0004, false, true));
		assert!(body_present(0., true, false));
		assert!(body_present(0., false, true));
	}

	#[test]
	fn forced_placements_ignore_the_window() {
		assert_eq!(
			resolve_direction(Placement::Down, 700., 24., 500., 0., 800., 8.),
			Direction::Down
		);
		assert_eq!(
			resolve_direction(Placement::Up, 10., 24., 500., 0., 800., 8.),
			Direction::Up
		);
	}

	#[test]
	fn auto_flips_up_only_when_down_does_not_fit() {
		let auto = |top, body_h, vp| resolve_direction(Placement::Auto, top, 24., body_h, 0., vp, 8.);
		assert_eq!(auto(100., 120., 800.), Direction::Down);
		assert_eq!(auto(700., 120., 800.), Direction::Up);
		// Neither fits, more room above.
		assert_eq!(auto(300., 500., 600.), Direction::Up);
		// Neither fits, more room below.
		assert_eq!(auto(40., 500., 400.), Direction::Down);
		// Exactly fitting counts as fitting.
		assert_eq!(auto(100., 800. - 124. - 8., 800.), Direction::Down);
		// The offset is part of the reach.
		assert_eq!(
			resolve_direction(Placement::Auto, 640., 24., 100., 0., 800., 8.),
			Direction::Down
		);
		assert_eq!(
			resolve_direction(Placement::Auto, 640., 24., 100., 50., 800., 8.),
			Direction::Up
		);
	}

	#[test]
	fn opening_keeps_present_and_closing_uses_the_rebounding_spring() {
		assert_eq!(body_spring(true), Curve::Present.spring().unwrap());
		assert_eq!(body_spring(false), MORPH_CLOSE);
		assert!(MORPH_CLOSE.response <= body_spring(true).response);
	}

	/// Steps the body spring the way `render` does, one frame at a time, and returns each
	/// frame's `(t, active)`.
	fn run(
		mut state: motion::SpringState,
		open: bool,
		frames: usize,
	) -> (Vec<(f32, bool)>, motion::SpringState) {
		let target = if open { 1. } else { 0. };
		let mut out = Vec::new();
		for _ in 0..frames {
			let (next, active) = motion::value::advance(state, target, body_spring(open), 1. / 120.);
			state = next;
			out.push((state.value, active));
			if !active {
				break;
			}
		}
		(out, state)
	}

	#[test]
	fn closing_dips_a_little_inside_the_trigger_and_settles_without_a_snap() {
		// A big body on a small round trigger, like the shortcut legend.
		let frame = |t: f32| {
			surface_frame(
				t,
				Direction::Down,
				Align::End,
				-28.,
				(28., 28.),
				(260., 300.),
				14.,
				20.,
			)
		};
		let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
		let (frames, end) = run(open, false, 600);
		assert_eq!(end, motion::SpringState::at_rest(0.));
		assert!(!frames.last().unwrap().1);
		// Never more than the cap inside the trigger, and the last frame is exactly its rect and radius.
		let all: Vec<Surface> = frames.iter().map(|(t, _)| frame(*t)).collect();
		for s in &all {
			let c = CLOSE_DIP_PX + 1e-4;
			assert!(s.width >= 28. - 2. * c && s.height >= 28. - 2. * c && s.left <= c && s.top <= c);
		}
		let last = all[all.len() - 1];
		assert_eq!(
			(last.left, last.top, last.width, last.height, last.radius),
			(0., 0., 28., 28., 14.)
		);
		let before = all[all.len() - 2];
		assert!(
			before.height - last.height < 0.35,
			"snap of {} px",
			before.height - last.height
		);
		// It falls monotonically until the dip, then rebounds by at most the cap.
		let low = all.iter().map(|s| s.height).fold(f32::MAX, f32::min);
		assert!((28. - 2. * CLOSE_DIP_PX - 1e-4..28.).contains(&low));
		let first_dip = all.iter().position(|s| s.height < 28.).expect("dips");
		assert!(
			all[..first_dip]
				.windows(2)
				.all(|w| w[1].height <= w[0].height + 1e-3)
		);
	}

	#[test]
	fn reversing_keeps_the_velocity() {
		let (frames, mid) = run(motion::SpringState::at_rest(0.), true, 20);
		assert!(mid.velocity > 0.);
		let (next, _) = motion::value::advance(mid, 0., body_spring(false), 1. / 120.);
		// The velocity bends the path instead of restarting it: still moving up one frame later.
		assert!(next.value > mid.value - 0.001);
		assert!(frames.len() == 20);
	}

	fn look(fill: Rgba, edge: Rgba, glass: bool) -> Look {
		let filter = glass.then(|| {
			crate::glass::Glass::new(None, crate::GlassStyle::MacOS26)
				.filter()
				.unwrap()
		});
		Look { filter, fill, edge }
	}

	#[test]
	fn look_gradient_hits_both_ends() {
		let from = look(white(0.2), white(0.4), true);
		let mut to = look(black(0.8), black(0.6), true);
		to.filter = crate::glass::Glass::new(Some(white(1.)), crate::GlassStyle::MacOS26)
			.elevation(Elevation::Raised)
			.filter();
		assert_eq!(from.mix(&to, 0.), from);
		assert_eq!(from.mix(&to, 1.), to);
		assert_eq!(from.mix(&to, -1.), from);
		assert_eq!(from.mix(&to, 3.), to);
		// The group's material at t = 0 is exactly what the trigger painted, light angle included.
		let mixed = from.mix(&to, 0.).filter.unwrap();
		assert_eq!(mixed, from.filter.unwrap());
		let mid = from.mix(&to, 0.5);
		assert!((mid.fill.alpha - 0.5).abs() < 1e-6);
		assert!((mid.edge.alpha - 0.5).abs() < 1e-6);
	}

	#[test]
	fn look_without_glass_has_no_filter() {
		let flat = look(white(0.2), white(0.4), false);
		let glass = look(black(0.8), black(0.6), true);
		assert_eq!(flat.mix(&glass, 0.5).filter, None);
		assert_eq!(glass.mix(&flat, 0.5).filter, None);
	}
}

/// The mounted behaviour: real layout, the group's reported shapes, and clicks.
#[cfg(test)]
mod window_tests {
	use std::{
		cell::{Cell, RefCell},
		time::{Duration, Instant},
	};

	use gpui::{Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext, point, size};

	use super::*;
	const TRIGGER: (f32, f32) = (120., 32.);
	const BODY: (f32, f32) = (200., 100.);

	#[derive(Default)]
	struct Probes {
		/// The trigger's content bounds and how often it painted.
		trigger: Cell<Option<Bounds<Pixels>>>,
		painted: Cell<u32>,
		/// Every state `open` took, in order.
		history: RefCell<Vec<bool>>,
	}

	struct Host {
		open: bool,
		probes: Rc<Probes>,
	}

	impl Host {
		fn set(&mut self, open: bool, cx: &mut Context<Self>) {
			self.open = open;
			self.probes.history.borrow_mut().push(open);
			cx.notify();
		}
	}

	impl Render for Host {
		fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
			let probes = self.probes.clone();
			let toggle = cx.listener(|this, _, _, cx| this.set(!this.open, cx));
			let view = cx.entity();
			// A block parent with an in-flow trigger: the layout that exposes absolute-position mistakes.
			div().p(px(40.)).child(
				attached_surface("test-attached")
					.open(self.open)
					.placement(Placement::Down)
					.trigger(move |_, _, _| {
						div()
							.id("test-trigger")
							.relative()
							.w(px(TRIGGER.0))
							.h(px(TRIGGER.1))
							.child("Pick")
							.child(
								canvas(
									{
										let probes = probes.clone();
										move |bounds, _, _| probes.trigger.set(Some(bounds))
									},
									move |_, _, _, _| probes.painted.set(probes.painted.get() + 1),
								)
								.absolute()
								.top_0()
								.left_0()
								.size_full(),
							)
							.on_click(toggle)
							.into_any_element()
					})
					.body(BODY, 16., |_, _| {
						div().w(px(BODY.0)).h(px(BODY.1)).child("rows").into_any_element()
					})
					.on_dismiss(move |_, cx| view.update(cx, |this, cx| this.set(false, cx))),
			)
		}
	}

	fn mount(cx: &mut TestAppContext) -> (Entity<Host>, Rc<Probes>, &mut VisualTestContext) {
		cx.update(crate::init);
		let probes = Rc::new(Probes::default());
		let (view, cx) = cx.add_window_view({
			let probes = probes.clone();
			move |_, _| Host { open: false, probes }
		});
		cx.update(|window, _| window.activate());
		draw(&view, cx);
		(view, probes, cx)
	}

	fn draw(view: &Entity<Host>, cx: &mut VisualTestContext) {
		view.update(cx, |_, cx| cx.notify());
		cx.update(|window, cx| window.draw(cx).clear(cx));
	}

	fn sweep(view: &Entity<Host>, cx: &mut VisualTestContext, mut each: impl FnMut()) {
		let start = Instant::now();
		while start.elapsed() < Duration::from_millis(1200) {
			draw(view, cx);
			each();
			std::thread::sleep(Duration::from_millis(4));
		}
	}

	/// The surface's outer rect as last painted (the probe sits inside its 1 px border).
	fn surface() -> Option<Bounds<Pixels>> {
		LAST_SURFACE.with(|last| {
			last.borrow().map(|inner| Bounds {
				origin: inner.origin - point(px(1.), px(1.)),
				size: inner.size + size(px(2.), px(2.)),
			})
		})
	}

	#[gpui::test]
	fn trigger_stays_and_one_surface_morphs_out_of_it(cx: &mut TestAppContext) {
		let (view, probes, cx) = mount(cx);
		let rest = probes.trigger.get().expect("trigger laid out");
		assert_eq!(rest.size, size(px(TRIGGER.0), px(TRIGGER.1)));
		assert!(surface().is_none(), "no surface at rest");

		let check = |opening: bool, cx: &mut VisualTestContext| {
			let mut grew = false;
			let mut last_painted = probes.painted.get();
			let eps = px(0.01);
			sweep(&view, cx, || {
				assert_eq!(probes.trigger.get(), Some(rest), "the trigger moved");
				assert!(
					probes.painted.get() > last_painted,
					"the trigger content was not painted"
				);
				last_painted = probes.painted.get();
				if let Some(s) = surface() {
					let cap = px(CLOSE_DIP_PX) + eps;
					assert!(s.origin.y - rest.origin.y <= cap && s.origin.x - rest.origin.x <= cap);
					assert!(s.bottom() >= rest.bottom() - cap && s.right() >= rest.right() - cap);
					assert!(
						s.origin.x >= rest.origin.x - eps && s.origin.y >= rest.origin.y - eps,
						"{s:?} starts left of or above {rest:?}"
					);
					assert!(s.size.width <= px(BODY.0 + 3.) && s.size.height <= px(TRIGGER.1 + BODY.1 + 3.));
					grew |= s.size.height > rest.size.height + px(1.);
				}
			});
			assert!(
				grew,
				"the surface never grew while {}",
				if opening { "opening" } else { "closing" }
			);
		};
		view.update(cx, |host, cx| host.set(true, cx));
		check(true, cx);
		let open = surface().expect("open surface");
		assert!(
			(f32::from(open.size.height) - (TRIGGER.1 + BODY.1)).abs() < 0.1,
			"{open:?}"
		);
		view.update(cx, |host, cx| host.set(false, cx));
		check(false, cx);
		assert!(surface().is_none(), "the surface leaves once settled closed");
	}

	#[gpui::test]
	fn clicking_the_trigger_while_open_closes_without_reopening(cx: &mut TestAppContext) {
		let (view, probes, cx) = mount(cx);
		let rest = probes.trigger.get().unwrap();
		let center = point(rest.center().x, rest.center().y);
		view.update(cx, |host, cx| host.set(true, cx));
		sweep(&view, cx, || {});
		cx.simulate_click(center, Modifiers::none());
		sweep(&view, cx, || {});
		assert_eq!(*probes.history.borrow(), [true, false], "toggled more than once");
	}
}
