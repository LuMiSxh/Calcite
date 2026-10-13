// SPDX-License-Identifier: MPL-2.0

//! Form controls: [`toggle`] (`SettingsToggle`, in the elongated macOS 26 shape), [`slider`] (`SettingsSlider`),
//! [`segmented`] (`AdaptiveSegmentedControl`), and [`text_field`] (`ThemedTextField`).
//!
//! The controls are stateless: the view owns the value and passes a change handler, like a
//! SwiftUI binding split into a value and a setter.
//!
//! ```ignore
//! toggle("auto-update", self.auto_update).label("Automatic updates")
//!     .on_change(cx.listener(|this, on: &bool, _, cx| { this.auto_update = *on; cx.notify(); }))
//! slider("volume", self.volume).range(0.0..=1.0).step(0.05)
//!     .on_change(cx.listener(|this, value: &f32, _, cx| { this.volume = *value; cx.notify(); }))
//! segmented("mode", self.mode, [(Mode::Windowed, "Windowed"), (Mode::Fullscreen, "Fullscreen")])
//!     .on_change(cx.listener(|this, mode: &Mode, _, cx| { this.mode = *mode; cx.notify(); }))
//! ```

use std::ops::RangeInclusive;

use gpui::{
	App, Bounds, ElementId, Entity, Filter, FocusHandle, Focusable, FontWeight, GlassFilter, IntoElement,
	MouseButton, ParentElement, Pixels, Role, SharedString, Styled, Window, canvas, div, prelude::*, px,
};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, text_input};

use crate::{
	controls::{Size, theme_override},
	glass::Elevation,
	icons::IconRef,
	interaction::{ControlInteraction, ControlState, Feedback, spring_value},
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{self, control, scale_alpha, white, with_alpha},
};

/// A shared change handler, for controls that hand it to several listeners.
pub(crate) type SharedChangeHandler<T> = std::rc::Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

type ChangeHandler<T> = Box<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// An accent switch in the elongated Liquid Glass shape of macOS 26 and 27: a capsule thumb
/// that turns into a clear glass lens while pressed and stretches as it travels.
#[derive(IntoElement)]
pub struct Toggle {
	id: ElementId,
	on: bool,
	label: SharedString,
	size: Size,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<ChangeHandler<bool>>,
}

/// A switch showing `on`. Give it a `.label(..)`: it is the accessible name.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Toggle {
	Toggle {
		id: id.into(),
		on,
		label: SharedString::default(),
		size: Size::Regular,
		enabled: true,
		theme: None,
		on_change: None,
	}
}

/// Track and thumb geometry. Large is the 64 × 28 track and 39 × 24 thumb of Apple's
/// iOS 26 and macOS 26 design kits; the other sizes keep its proportions.
struct SwitchMetrics {
	track: (f32, f32),
	thumb: (f32, f32),
}

impl SwitchMetrics {
	fn of(size: Size) -> Self {
		match size {
			Size::Large => Self {
				track: (64., 28.),
				thumb: (39., 24.),
			},
			Size::Regular => Self {
				track: (48., 22.),
				thumb: (29., 18.),
			},
			Size::Compact => Self {
				track: (38., 18.),
				thumb: (23., 14.),
			},
		}
	}
}

/// How far the pressed lens grows past the thumb, horizontally and vertically.
const LENS_GROWTH: (f32, f32) = (0.42, 0.55);
/// How far the thumb stretches at the middle of its travel.
const TRAVEL_STRETCH: f32 = 0.24;

impl Toggle {
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	pub fn size(mut self, size: Size) -> Self {
		self.size = size;
		self
	}

	pub fn large(self) -> Self {
		self.size(Size::Large)
	}

	pub fn compact(self) -> Self {
		self.size(Size::Compact)
	}

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Toggle {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let state = ControlState::of(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::PressOnly,
			self.enabled,
			reduce_motion,
			window,
			cx,
		);
		let target = if self.on { 1. } else { 0. };
		let position = spring_value(
			(self.id.clone(), "thumb"),
			target,
			Curve::Release,
			reduce_motion,
			window,
			cx,
		);
		let lens = spring_value(
			(self.id.clone(), "lens"),
			if frame.pressed && !reduce_motion { 1. } else { 0. },
			if frame.pressed {
				Curve::Press
			} else {
				Curve::Release
			},
			reduce_motion,
			window,
			cx,
		)
		.max(0.);

		let SwitchMetrics {
			track: (track_width, track_height),
			thumb: (thumb_width, thumb_height),
		} = SwitchMetrics::of(self.size);
		let inset = (track_height - thumb_height) / 2.;
		let travel = track_width - thumb_width - 2. * inset;
		// The stretch peaks halfway between the ends and vanishes at rest.
		let remaining = (target - position).abs().min(1.);
		let stretch = if reduce_motion {
			0.
		} else {
			4. * remaining * (1. - remaining)
		};
		let width = thumb_width * (1. + LENS_GROWTH.0 * lens + TRAVEL_STRETCH * stretch);
		let height = thumb_height * (1. + LENS_GROWTH.1 * lens);
		let center = inset + thumb_width / 2. + travel * position.clamp(-0.08, 1.08);

		let off_track = if theme.reduce_transparency() {
			theme.palette.opaque_surface
		} else {
			white(0.16)
		};
		let track = tokens::mix(off_track, theme.accent, position.clamp(0., 1.));
		let alpha = if self.enabled {
			1.
		} else {
			control::DISABLED_FOREGROUND_OPACITY
		};
		let thumb = div()
			.absolute()
			.left(px(center - width / 2.))
			.top(px((track_height - height) / 2.))
			.w(px(width))
			.h(px(height))
			.rounded_full();
		let thumb = if lens > 0.01 && !theme.reduce_transparency() {
			// A clear lens: the solid thumb fades out while the glass bends the track below.
			let fill = white(0.06 + 0.94 * (1. - lens).powi(2));
			thumb
				.backdrop_filter(vec![Filter::Glass(GlassFilter {
					blur: px(0.6),
					bezel: px(height * 0.38),
					refraction: px(height * 0.22 * lens),
					dispersion: 0.3,
					saturation: 1.25,
					brightness: 1.08,
					fill: [fill.red, fill.green, fill.blue, fill.alpha],
					rim: 0.9,
					specular: 0.85,
					..GlassFilter::default()
				})])
				.shadow(vec![
					gpui::BoxShadow::new(px(0.), px(2.), tokens::black(0.22 * lens)).blur_radius(px(8.)),
				])
		} else {
			thumb.bg(scale_alpha(white(1.), alpha)).shadow(vec![
				gpui::BoxShadow::new(px(0.), px(1.), tokens::black(0.28)).blur_radius(px(3.)),
			])
		};
		let on = self.on;
		div()
			.id(self.id)
			.relative()
			.w(px(track_width))
			.h(px(track_height))
			.flex_none()
			.rounded_full()
			.bg(scale_alpha(track, alpha))
			.role(Role::Switch)
			.aria_label(self.label)
			.control_state(&state, false)
			.child(thumb)
			.when(self.enabled, |this| this.cursor_pointer())
			.when_some(self.on_change.filter(|_| self.enabled), move |this, on_change| {
				this.on_click(move |_, window, cx| on_change(&!on, window, cx))
			})
	}
}

/// A horizontal value slider with an accent fill.
#[derive(IntoElement)]
pub struct Slider {
	id: ElementId,
	value: f32,
	range: RangeInclusive<f32>,
	step: Option<f32>,
	width: Pixels,
	label: SharedString,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<SharedChangeHandler<f32>>,
}

/// A slider showing `value` in `0.0..=1.0` unless `.range(..)` says otherwise.
pub fn slider(id: impl Into<ElementId>, value: f32) -> Slider {
	Slider {
		id: id.into(),
		value,
		range: 0.0..=1.0,
		step: None,
		width: px(180.),
		label: SharedString::default(),
		enabled: true,
		theme: None,
		on_change: None,
	}
}

impl Slider {
	pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
		self.range = range;
		self
	}

	pub fn step(mut self, step: f32) -> Self {
		self.step = Some(step);
		self
	}

	pub fn width(mut self, width: Pixels) -> Self {
		self.width = width;
		self
	}

	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}
}

/// Maps a pointer x position to a stepped value inside `range`.
fn slider_value(x: Pixels, bounds: Bounds<Pixels>, range: &RangeInclusive<f32>, step: Option<f32>) -> f32 {
	let width = f32::from(bounds.size.width).max(1.);
	let fraction = (f32::from(x - bounds.origin.x) / width).clamp(0., 1.);
	let (start, end) = (*range.start(), *range.end());
	let mut value = start + (end - start) * fraction;
	if let Some(step) = step.filter(|step| *step > 0.) {
		value = start + ((value - start) / step).round() * step;
	}
	value.clamp(start.min(end), start.max(end))
}

impl RenderOnce for Slider {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let bounds = window.use_keyed_state((self.id.clone(), "track"), cx, |_, _| None::<Bounds<Pixels>>);
		let (start, end) = (*self.range.start(), *self.range.end());
		let fraction = if end == start {
			0.
		} else {
			((self.value - start) / (end - start)).clamp(0., 1.)
		};
		let alpha = if self.enabled {
			1.
		} else {
			control::DISABLED_FOREGROUND_OPACITY
		};
		let thumb = 16.;
		let track_width = f32::from(self.width);
		let probe = bounds.clone();
		let mut slider = div()
			.id(self.id)
			.relative()
			.w(self.width)
			.h(px(thumb + 4.))
			.flex_none()
			.role(Role::Slider)
			.aria_label(self.label)
			.child(
				div()
					.absolute()
					.left_0()
					.right_0()
					.top(px(thumb / 2.))
					.h(px(4.))
					.rounded_full()
					.bg(white(0.18 * alpha)),
			)
			.child(
				div()
					.absolute()
					.left_0()
					.top(px(thumb / 2.))
					.w(px(track_width * fraction))
					.h(px(4.))
					.rounded_full()
					.bg(scale_alpha(theme.accent, alpha)),
			)
			.child(
				div()
					.absolute()
					.top(px(2.))
					.left(px((track_width - thumb) * fraction))
					.size(px(thumb))
					.rounded_full()
					.bg(scale_alpha(white(1.), alpha))
					.shadow(vec![
						gpui::BoxShadow::new(px(0.), px(1.), tokens::black(0.35)).blur_radius(px(3.)),
					]),
			)
			.child(
				canvas(
					move |area, _, cx| probe.update(cx, |bounds, _| *bounds = Some(area)),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			);
		if let (true, Some(on_change)) = (self.enabled, self.on_change) {
			let (range, step) = (self.range.clone(), self.step);
			let press = (bounds.clone(), on_change.clone(), range.clone());
			slider = slider
				.cursor_pointer()
				.on_mouse_down(MouseButton::Left, move |event, window, cx| {
					if let Some(area) = *press.0.read(cx) {
						(press.1)(&slider_value(event.position.x, area, &press.2, step), window, cx);
					}
				})
				.on_mouse_move(move |event, window, cx| {
					if event.pressed_button != Some(MouseButton::Left) {
						return;
					}
					if let Some(area) = *bounds.read(cx) {
						on_change(&slider_value(event.position.x, area, &range, step), window, cx);
					}
				});
		}
		slider
	}
}

/// A row of mutually exclusive options; the selection carries the accent glass.
#[derive(IntoElement)]
pub struct Segmented<T: Clone + PartialEq + 'static> {
	id: ElementId,
	selected: T,
	options: Vec<(T, SharedString)>,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<SharedChangeHandler<T>>,
}

/// A segmented control over `options`, showing `selected`.
pub fn segmented<T: Clone + PartialEq + 'static, L: Into<SharedString>>(
	id: impl Into<ElementId>,
	selected: T,
	options: impl IntoIterator<Item = (T, L)>,
) -> Segmented<T> {
	Segmented {
		id: id.into(),
		selected,
		options: options
			.into_iter()
			.map(|(value, label)| (value, label.into()))
			.collect(),
		enabled: true,
		theme: None,
		on_change: None,
	}
}

impl<T: Clone + PartialEq + 'static> Segmented<T> {
	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}

	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

impl<T: Clone + PartialEq + 'static> RenderOnce for Segmented<T> {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = tokens::contrast_tint(theme.accent);
		let reduced = theme.reduce_transparency();
		let container_fill = if reduced {
			theme.palette.opaque_surface
		} else {
			white(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
		};
		let selected_glass = theme
			.surface(Some(with_alpha(
				theme.accent,
				control::ENABLED_CHROMATIC_SURFACE_OPACITY,
			)))
			.elevation(Elevation::None);
		let enabled = self.enabled;
		let id = self.id.clone();
		div()
			.id(self.id)
			.flex()
			.flex_row()
			.gap(px(4.))
			.p(px(4.))
			.rounded_full()
			.bg(container_fill)
			.border_1()
			.border_color(with_alpha(
				theme.palette.control,
				control::ENABLED_NEUTRAL_BORDER_OPACITY,
			))
			.children(
				self.options
					.into_iter()
					.enumerate()
					.map(|(index, (value, label))| {
						let selected = value == self.selected;
						let on_change = self.on_change.clone();
						let foreground = if !enabled {
							with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
						} else if selected {
							accent
						} else {
							theme.palette.secondary_text
						};
						let segment = div()
							.id((id.clone(), SharedString::from(index.to_string())))
							.flex_1()
							.flex()
							.items_center()
							.justify_center()
							.px(px(10.))
							.py(px(5.))
							.rounded_full()
							.text_size(tokens::text::CAPTION)
							.font_weight(FontWeight::SEMIBOLD)
							.text_color(foreground)
							.role(Role::Button)
							.aria_label(label.clone())
							.child(label);
						let segment = if selected {
							let segment = segment.border_1().border_color(with_alpha(
								theme.accent,
								control::ENABLED_CHROMATIC_BORDER_OPACITY,
							));
							match selected_glass.filter() {
								Some(filter) => segment.backdrop_filter(vec![Filter::Glass(filter)]),
								None => segment.bg(selected_glass.fill()),
							}
						} else {
							segment.hover(|style| style.bg(white(0.04)))
						};
						segment.when_some(
							on_change.filter(|_| enabled && !selected),
							move |segment, on_change| {
								segment
									.cursor_pointer()
									.on_click(move |_, window, cx| on_change(&value, window, cx))
							},
						)
					}),
			)
	}
}

/// A single-line input in the accent capsule (`ThemedTextField`).
///
/// The text lives in an [`EditableTextState`] the view owns; create it with
/// [`text_state`] and read it with `state.read(cx).as_str()`, or subscribe to its
/// `TextChanged` event.
#[derive(IntoElement)]
pub struct TextField {
	id: ElementId,
	state: Entity<EditableTextState>,
	placeholder: SharedString,
	icon: Option<IconRef>,
	width: Option<Pixels>,
	theme: Option<Theme>,
}

/// A text field editing `state`.
pub fn text_field(id: impl Into<ElementId>, state: &Entity<EditableTextState>) -> TextField {
	TextField {
		id: id.into(),
		state: state.clone(),
		placeholder: SharedString::default(),
		icon: None,
		width: None,
		theme: None,
	}
}

/// Creates the state of a text field with `initial` text.
pub fn text_state(initial: &str, cx: &mut App) -> Entity<EditableTextState> {
	let storage = StringStorage::from(initial);
	cx.new(|cx| EditableTextState::new(storage, cx))
}

impl TextField {
	pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
		self.placeholder = placeholder.into();
		self
	}

	pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	pub fn width(mut self, width: Pixels) -> Self {
		self.width = Some(width);
		self
	}
}

impl RenderOnce for TextField {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let focus: FocusHandle = self.state.read(cx).focus_handle(cx);
		let focused = focus.is_focused(window);
		let glass = theme
			.surface(Some(with_alpha(
				theme.accent,
				control::ENABLED_CHROMATIC_SURFACE_OPACITY,
			)))
			.elevation(Elevation::None);
		let border = if focused {
			theme.accent
		} else {
			with_alpha(theme.accent, control::ENABLED_CHROMATIC_BORDER_OPACITY)
		};
		let icon_color = if focused {
			tokens::contrast_tint(theme.accent)
		} else {
			theme.palette.secondary_text
		};
		let field = div()
			.id(self.id.clone())
			.flex()
			.flex_row()
			.items_center()
			.gap(px(7.))
			.px(px(control::COMPACT_HORIZONTAL_PADDING))
			.py(px(control::COMPACT_VERTICAL_PADDING + 2.))
			.rounded_full()
			.border_1()
			.border_color(border)
			.text_size(px(12.))
			.text_color(theme.palette.primary_text)
			.when_some(self.width, |field, width| field.w(width))
			.when_some(self.icon, |field, icon| {
				field.child(icon.render(px(11.), icon_color))
			})
			.child(
				text_input((self.id, "input"))
					.state(self.state.downgrade())
					.placeholder(self.placeholder)
					.placeholder_color(theme.palette.secondary_text)
					.caret_color(gpui::rgb_to_hsla(tokens::contrast_tint(theme.accent)))
					.selection_color(gpui::rgb_to_hsla(scale_alpha(theme.accent, 0.35)))
					.flex_1()
					.min_w_0()
					.whitespace_nowrap()
					.overflow_x_scroll(),
			);
		match glass.filter() {
			Some(filter) => field.backdrop_filter(vec![Filter::Glass(filter)]),
			None => field.bg(glass.fill()),
		}
	}
}

theme_override!(Toggle, Slider, TextField);
