// SPDX-License-Identifier: MPL-2.0

//! Morphing glass: several surfaces rendered by one glass pass so that neighbours fuse, like
//! SwiftUI's `GlassEffectContainer`.
//!
//! A [`glass_group`] is a container. Its members are ordinary [`Div`]s declared with
//! [`GlassContainer::member`]; they draw no glass themselves. Each frame the members report their
//! laid-out bounds (in the prepaint phase, which runs before any painting), and the container
//! paints one combined [`gpui::GlassGroup`] behind its children. Because the bounds are read in
//! the same frame, a member whose size or position animates (`spring_value`) drags the fused
//! field with it, without a frame of lag.
//!
//! The group owns the look of its members: one glass fill (the HUD surface unless `tint` says
//! otherwise) and one hairline outline around the union of the shapes, so neighbours that fuse
//! share a single outline and a uniform fill. [`GlassContainer::member`] therefore strips the
//! border and background of the element it is given; do not add them to members.
//!
//! Fallbacks: under Reduce Transparency members get a plain fill (no morph), and members past
//! [`MAX_GLASS_SHAPES`] paint their own single-shape glass.

use std::{cell::RefCell, rc::Rc};

use gpui::{
	AnyElement, App, Bounds, Corners, Div, ElementId, GlassFilter, GlassGroup as GpuiGlassGroup, GlassShape,
	InteractiveElement, IntoElement, MAX_GLASS_SHAPES, ParentElement, Pixels, RenderOnce, Rgba,
	StyleRefinement, Styled, Window, canvas, div, px, transparent_black,
};

use crate::{
	glass::{Elevation, Glass, GlassExt},
	theme::{ActiveTheme, Theme},
};

// The shapes of the most recent group paint on this thread, for tests.
#[cfg(test)]
thread_local! {
	pub(crate) static LAST_PAINTED: RefCell<Vec<Bounds<Pixels>>> = const { RefCell::new(Vec::new()) };
}

/// The shapes members reported this frame, in tree order.
#[derive(Default)]
struct Shapes(Vec<GlassShape>);

impl Shapes {
	fn push(&mut self, bounds: Bounds<Pixels>, corner_radii: Corners<Pixels>) {
		self.0.push(GlassShape { bounds, corner_radii });
	}

	fn clear(&mut self) {
		self.0.clear();
	}

	/// The combined group. Shapes beyond [`MAX_GLASS_SHAPES`] are dropped, empty ones skipped.
	fn group(&self, glass: GlassFilter, spacing: Pixels) -> GpuiGlassGroup {
		self.0
			.iter()
			.filter(|shape| shape.bounds.size.width > px(0.) && shape.bounds.size.height > px(0.))
			.take(MAX_GLASS_SHAPES)
			.fold(GpuiGlassGroup::new(glass).spacing(spacing), |group, shape| {
				group.shape(shape.bounds, shape.corner_radii)
			})
	}
}

/// For each of `members` in order, whether it joins the fused pass. With no glass (Reduce
/// Transparency) none does; past the shape limit the rest paint their own glass.
fn fused_flags(members: usize, glass: bool) -> Vec<bool> {
	(0..members)
		.map(|index| glass && index < MAX_GLASS_SHAPES)
		.collect()
}

/// The group's material: the HUD surface unless a tint was given.
fn resolve_glass(theme: &Theme, tint: Option<Option<Rgba>>, elevation: Elevation) -> Glass {
	match tint {
		None => theme.hud_surface(),
		Some(tint) => theme.surface(tint),
	}
	.elevation(elevation)
}

enum Item {
	Plain(AnyElement),
	Member(Box<dyn FnOnce(Mode) -> AnyElement>),
}

/// How a member is drawn.
enum Mode {
	/// Reports its bounds to the shared pass.
	Fused(Rc<RefCell<Shapes>>),
	/// Paints its own glass or plain fill.
	Own(crate::glass::Glass, Rgba),
}

/// A container whose members fuse into one piece of glass. See the [module docs](self).
#[derive(IntoElement)]
pub struct GlassContainer {
	id: ElementId,
	base: Div,
	spacing: Pixels,
	/// `None` is the HUD tint; `Some(None)` is untinted glass.
	tint: Option<Option<Rgba>>,
	elevation: Elevation,
	material: Option<(GlassFilter, Rgba)>,
	theme: Option<Theme>,
	items: Vec<Item>,
}

/// A glass container: members declared with [`GlassContainer::member`] share one glass pass and
/// fuse when closer than `spacing`. The container is a `Div` for layout and styling.
pub fn glass_group(id: impl Into<ElementId>, spacing: Pixels) -> GlassContainer {
	GlassContainer {
		id: id.into(),
		base: div(),
		spacing,
		tint: None,
		elevation: Elevation::Resting,
		material: None,
		theme: None,
		items: Vec::new(),
	}
}

impl GlassContainer {
	/// The tint of the shared glass, as for `Theme::surface`. The default is the HUD tint, so a
	/// group looks like a standalone `Theme::hud_surface` pill.
	pub fn tint(mut self, tint: Option<Rgba>) -> Self {
		self.tint = Some(tint);
		self
	}

	/// The lens depth of the shared glass. The group paints no shadow; give members their own.
	pub fn elevation(mut self, elevation: Elevation) -> Self {
		self.elevation = elevation;
		self
	}

	pub fn spacing(mut self, spacing: Pixels) -> Self {
		self.spacing = spacing;
		self
	}

	/// An explicit material and outline colour for the whole group, replacing the tint and
	/// elevation. For a group whose look is animated, such as [`crate::attached`]. Under
	/// Reduce Transparency the group is expected to be skipped by the caller.
	pub fn material(mut self, filter: GlassFilter, edge: Rgba) -> Self {
		self.material = Some((filter, edge));
		self
	}

	/// A surface of the group that keeps its own border, background, and layout: it only
	/// reports its bounds (with corner `radius`) to the shared pass. Use for a real control
	/// that must stay exactly as it is while it fuses with a neighbour.
	pub fn fused<E>(mut self, radius: Pixels, element: E) -> Self
	where
		E: ParentElement + IntoElement + 'static,
	{
		self.items.push(Item::Member(Box::new(move |mode| match mode {
			Mode::Fused(shapes) => element
				.child(
					canvas(
						move |bounds, _, _| shapes.borrow_mut().push(bounds, Corners::all(radius)),
						|_, _, _, _| {},
					)
					.absolute()
					.top_0()
					.left_0()
					.size_full(),
				)
				.into_any_element(),
			Mode::Own(..) => element.into_any_element(),
		})));
		self
	}

	/// A glass member: `element` becomes a surface of the group with corner `radius`. The group
	/// owns fill and outline, so any border or background on `element` is removed; a shadow or
	/// content is up to you. Children and members paint in the order they are added, over the
	/// shared glass.
	pub fn member<E>(mut self, radius: Pixels, element: E) -> Self
	where
		E: Styled + ParentElement + IntoElement + 'static,
	{
		self.items.push(Item::Member(Box::new(move |mode| {
			let element = element.rounded(radius).border_0().bg(transparent_black());
			match mode {
				Mode::Fused(shapes) => element
					.child(
						canvas(
							move |bounds, _, _| shapes.borrow_mut().push(bounds, Corners::all(radius)),
							|_, _, _, _| {},
						)
						.absolute()
						.top_0()
						.left_0()
						.size_full(),
					)
					.into_any_element(),
				Mode::Own(glass, edge) => element
					.glass(glass)
					.border_1()
					.border_color(edge)
					.into_any_element(),
			}
		})));
		self
	}

	/// An ordinary child, painted over the glass.
	pub fn child(mut self, child: impl IntoElement) -> Self {
		self.items.push(Item::Plain(child.into_any_element()));
		self
	}
}

impl Styled for GlassContainer {
	fn style(&mut self) -> &mut StyleRefinement {
		self.base.style()
	}
}

impl RenderOnce for GlassContainer {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let glass = resolve_glass(&theme, self.tint, self.elevation);
		let (filter, edge) = match self.material {
			Some((filter, edge)) => (Some(filter), edge),
			None => (glass.filter(), glass.edge(theme.palette.hairline)),
		};
		let member_count = self
			.items
			.iter()
			.filter(|item| matches!(item, Item::Member(_)))
			.count();
		let mut flags = fused_flags(member_count, filter.is_some()).into_iter();
		let shapes = Rc::new(RefCell::new(Shapes::default()));

		let mut root = self.base.id(self.id);
		if let Some(filter) = filter {
			let reset = shapes.clone();
			let read = shapes.clone();
			let spacing = self.spacing;
			let border = [edge.red, edge.green, edge.blue, edge.alpha];
			// First child, so it paints before every member and plain child.
			root = root.child(
				canvas(
					move |_, _, _| reset.borrow_mut().clear(),
					move |_, _, window, _| {
						#[cfg(test)]
						LAST_PAINTED.with(|last| {
							*last.borrow_mut() = read.borrow().0.iter().map(|shape| shape.bounds).collect();
						});
						window
							.paint_glass_group(&read.borrow().group(filter, spacing).border(border, px(1.)));
					},
				)
				.absolute()
				.top_0()
				.left_0()
				.size_full(),
			);
		}
		for item in self.items {
			root = match item {
				Item::Plain(child) => root.child(child),
				Item::Member(build) => {
					let mode = if flags.next().unwrap_or(false) {
						Mode::Fused(shapes.clone())
					} else {
						Mode::Own(glass, edge)
					};
					root.child(build(mode))
				}
			};
		}
		root
	}
}

#[cfg(test)]
mod tests {
	use gpui::{point, size};

	use super::*;

	#[test]
	fn default_group_glass_matches_a_standalone_hud_pill() {
		let theme = Theme::default();
		let group = resolve_glass(&theme, None, Elevation::Resting);
		let pill = theme.hud_surface();
		assert_eq!(group.fill(), pill.fill());
		assert_eq!(
			group.edge(theme.palette.hairline),
			pill.edge(theme.palette.hairline)
		);
		let untinted = resolve_glass(&theme, Some(None), Elevation::Resting);
		assert_eq!(untinted.tint, None);
	}

	#[test]
	fn edge_is_the_hud_hairline_unless_glass_draws_its_own() {
		let theme = Theme::default();
		let hairline = theme.palette.hairline;
		let glass = theme.hud_surface();
		let expected = match glass.border() {
			Some(own) => own,
			None => crate::tokens::scale_alpha(hairline, crate::tokens::control::HUD_BORDER_OPACITY),
		};
		assert_eq!(glass.edge(hairline), expected);
		let flat = Theme::default().with_glass(crate::GlassStyle::ReducedTransparency);
		assert_eq!(
			flat.hud_surface().edge(hairline),
			flat.hud_surface().border().unwrap()
		);
	}

	fn glass() -> GlassFilter {
		crate::glass::Glass::new(None, crate::GlassStyle::MacOS26)
			.filter()
			.unwrap()
	}

	fn bounds(x: f32, w: f32) -> Bounds<Pixels> {
		Bounds::new(point(px(x), px(0.)), size(px(w), px(20.)))
	}

	#[test]
	fn shapes_keep_report_order() {
		let mut shapes = Shapes::default();
		shapes.push(bounds(0., 10.), Corners::all(px(4.)));
		shapes.push(bounds(30., 10.), Corners::all(px(6.)));
		let group = shapes.group(glass(), px(12.));
		assert_eq!(group.shapes.len(), 2);
		assert_eq!(group.shapes[1].bounds.origin.x, px(30.));
		assert_eq!(group.shapes[0].corner_radii, Corners::all(px(4.)));
		assert_eq!(group.spacing, px(12.));
	}

	#[test]
	fn shapes_are_capped_and_empty_ones_skipped() {
		let mut shapes = Shapes::default();
		shapes.push(bounds(0., 0.), Corners::all(px(0.)));
		for i in 0..12 {
			shapes.push(bounds(i as f32 * 20., 10.), Corners::all(px(4.)));
		}
		let group = shapes.group(glass(), px(8.));
		assert_eq!(group.shapes.len(), MAX_GLASS_SHAPES);
		assert_eq!(group.shapes[0].bounds.origin.x, px(0.));
		shapes.clear();
		assert!(shapes.group(glass(), px(8.)).shapes.is_empty());
	}

	#[test]
	fn extra_members_and_missing_glass_fall_back() {
		let flags = fused_flags(10, true);
		assert_eq!(flags.iter().filter(|fused| **fused).count(), MAX_GLASS_SHAPES);
		assert!(flags[..MAX_GLASS_SHAPES].iter().all(|fused| *fused));
		assert!(!flags[MAX_GLASS_SHAPES]);
		assert!(fused_flags(3, false).iter().all(|fused| !*fused));
	}
}
