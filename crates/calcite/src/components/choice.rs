// SPDX-License-Identifier: MPL-2.0

//! Setup questions and their progress: [`choice_card`] (`OnboardingAnswer`),
//! [`choice_group`] (`OnboardingQuestion`), [`tag`], and [`step_rail`] with [`step`]
//! (`OnboardingProgressRail`).
//!
//! A card picks one answer, toggles one of several answers, or runs an action. The app keeps
//! the selection and sets `.selected(..)` on each render.
//!
//! ```ignore
//! choice_group("Which server do you play on?", Icon::Globe)
//!     .child(choice_card("global", "Global").icon(Icon::Globe).detail("Yostar")
//!         .accessory(tag("Recommended")).selected(self.region == Region::Global)
//!         .on_click(cx.listener(|this, _, _, cx| ..)))
//! ```

use gpui::{
	AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement, Rgba, Role, SharedString,
	Styled, Toggled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ClickHandler, ThemeOverride, theme_override},
	icons::{Icon, IconRef},
	interaction::{
		ClickableRow, ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, ScaleExt,
		focus_handle, glow_enabled, glow_layer, layer, spring_value,
	},
	layout::settings_panel,
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{self, Chromatic, control, over, scale_alpha, white},
};

/// The trailing radio, checkbox, or chevron, and the size a filled glyph pops from.
const INDICATOR: f32 = 18.;
const INDICATOR_POP_FROM: f32 = 0.6;

/// How a card responds to a click, and which trailing indicator it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChoiceKind {
	/// One answer of a group: a radio.
	#[default]
	Single,
	/// Any number of answers: a checkbox.
	Multiple,
	/// Runs its own action: a chevron, no selection.
	Action,
}

impl ChoiceKind {
	fn indicator(self, selected: bool) -> Icon {
		match (self, selected) {
			(Self::Single, true) => Icon::CircleCheckFill,
			(Self::Single, false) => Icon::Circle,
			(Self::Multiple, true) => Icon::SquareCheckFill,
			(Self::Multiple, false) => Icon::Square,
			(Self::Action, _) => Icon::ChevronRight,
		}
	}

	fn role(self) -> Role {
		match self {
			Self::Single => Role::RadioButton,
			Self::Multiple => Role::CheckBox,
			Self::Action => Role::Button,
		}
	}
}

/// One answer card: icon, title, optional detail, and a trailing indicator.
#[derive(IntoElement)]
pub struct ChoiceCard {
	id: ElementId,
	title: SharedString,
	icon: Option<IconRef>,
	detail: Option<SharedString>,
	accessory: Option<AnyElement>,
	kind: ChoiceKind,
	selected: bool,
	enabled: bool,
	glow: Option<bool>,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// An answer card. Defaults: single choice, not selected, enabled.
pub fn choice_card(id: impl Into<ElementId>, title: impl Into<SharedString>) -> ChoiceCard {
	ChoiceCard {
		id: id.into(),
		title: title.into(),
		icon: None,
		detail: None,
		accessory: None,
		kind: ChoiceKind::Single,
		selected: false,
		enabled: true,
		glow: None,
		theme: None,
		on_click: None,
	}
}

impl ChoiceCard {
	pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	/// An element after the title, for example a [`tag`].
	pub fn accessory(mut self, accessory: impl IntoElement) -> Self {
		self.accessory = Some(accessory.into_any_element());
		self
	}

	pub fn kind(mut self, kind: ChoiceKind) -> Self {
		self.kind = kind;
		self
	}

	/// Action cards ignore the selection.
	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
		self
	}

	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	/// Forces the pointer light on or off. Without it the card follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow).
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for ChoiceCard {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let palette = theme.palette;
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::Subtle,
			self.enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let selected = self.selected && self.kind != ChoiceKind::Action;
		let accent = tokens::contrast_tint(theme.accent);
		let (fill, border) = if selected {
			let accent = Chromatic::enabled(theme.accent);
			(over(accent.fill, palette.panel_background), accent.border)
		} else {
			(palette.panel_background, palette.panel_border)
		};
		// `OnboardingAnswerBody` adds the hover fill once for hover and once more for a press.
		let hover = frame.hover_fill(white(1.));
		let fill = over(hover, fill);
		let fill = if frame.pressed {
			over(white(control::HOVER_OPACITY), fill)
		} else {
			fill
		};
		let indicator_color = if selected { accent } else { palette.secondary_text };
		let reduce_motion = theme.reduce_motion;
		// The filled glyph pops in on the release spring, so a choice reads as a press.
		let pop = spring_value(
			(self.id.clone(), "calcite-choice-pop"),
			if selected { 1. } else { 0. },
			Curve::Release,
			reduce_motion,
			window,
			cx,
		);
		let indicator = {
			let empty = self.kind.indicator(false);
			let filled = self.kind.indicator(true);
			let filled_size = INDICATOR * (INDICATOR_POP_FROM + (1. - INDICATOR_POP_FROM) * pop.max(0.));
			div()
				.relative()
				.size(px(INDICATOR))
				.flex_none()
				.flex()
				.items_center()
				.justify_center()
				.child(
					div()
						.absolute()
						.opacity((1. - pop).clamp(0., 1.))
						.child(empty.render(px(INDICATOR), palette.secondary_text)),
				)
				.when(self.kind != ChoiceKind::Action && pop > 0.01, |indicator| {
					indicator.child(
						div()
							.absolute()
							.opacity(pop.clamp(0., 1.))
							.child(filled.render(px(filled_size), accent)),
					)
				})
		};

		// A plain fill, not glass: glass rebuilt on a selection change snaps the release spring.
		let radius = theme.style.radii.row;
		let glows = self.enabled && glow_enabled(&theme, self.glow, false);
		let glow = glows
			.then(|| glow_layer(&frame, palette.primary_text))
			.flatten()
			.map(|glow| glow.rounded(radius).rounded_smoothing_ios());
		let surface = layer()
			.rounded(radius)
			.rounded_smoothing_ios()
			.bg(fill)
			.border_1()
			.border_color(border)
			.transitions(move |transitions| {
				transitions
					.bg(Curve::State.transition(reduce_motion))
					.border_color(Curve::State.transition(reduce_motion))
			});
		let label = div()
			.flex_1()
			.min_w_0()
			.flex()
			.flex_col()
			.gap(px(3.))
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(6.))
					.child(
						div()
							.font_weight(FontWeight::SEMIBOLD)
							.text_color(palette.primary_text)
							.child(self.title.clone()),
					)
					.children(self.accessory),
			)
			.when_some(self.detail.clone(), |label, detail| {
				label.child(
					div()
						.text_size(tokens::text::SUPPORTING)
						.text_color(palette.secondary_text)
						.child(detail),
				)
			});
		let content = div()
			.relative()
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(12.))
			.when_some(self.icon, |content, icon| {
				content.child(
					div()
						.w(px(28.))
						.flex()
						.justify_center()
						.child(icon.render(px(18.), indicator_color)),
				)
			})
			.child(label)
			.child(indicator);

		let card = div()
			.id(self.id)
			.relative()
			.w_full()
			.p(px(12.))
			.text_size(tokens::text::BODY)
			.font_family(theme.sans())
			.role(self.kind.role())
			.aria_label(self.title)
			.when_some(self.detail, |card, detail| card.aria_description(detail))
			.when(self.kind != ChoiceKind::Action, |card| {
				card.aria_toggled(if selected { Toggled::True } else { Toggled::False })
			})
			.child(surface)
			.children(glow)
			.child(content);
		if !self.enabled {
			return card
				.cursor_default()
				.aria_disabled(true)
				.opacity(control::DISABLED_FOREGROUND_OPACITY)
				.scaled(frame.scale);
		}
		card.keyboard_focus(&focus, FocusShape::Rounded(radius), &theme, window)
			.control_state(&state, glows)
			.when_some(self.on_click, |card, on_click| card.on_click(on_click))
			.scaled(frame.scale)
	}
}

/// A question panel: a titled header above a stack of [`choice_card`]s.
#[derive(IntoElement)]
pub struct ChoiceGroup {
	title: SharedString,
	icon: IconRef,
	accessory: Option<AnyElement>,
	footnote: Option<SharedString>,
	children: SmallVec<[AnyElement; 4]>,
	theme: Option<Theme>,
}

/// A question panel. Add cards with `.child(..)`.
pub fn choice_group(title: impl Into<SharedString>, icon: impl Into<IconRef>) -> ChoiceGroup {
	ChoiceGroup {
		title: title.into(),
		icon: icon.into(),
		accessory: None,
		footnote: None,
		children: SmallVec::new(),
		theme: None,
	}
}

impl ChoiceGroup {
	/// An element above the cards, for example a preview of the current choice.
	pub fn accessory(mut self, accessory: impl IntoElement) -> Self {
		self.accessory = Some(accessory.into_any_element());
		self
	}

	/// A hint under the cards.
	pub fn footnote(mut self, footnote: impl Into<SharedString>) -> Self {
		self.footnote = Some(footnote.into());
		self
	}
}

impl ParentElement for ChoiceGroup {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for ChoiceGroup {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let id = ElementId::Name(self.title.clone());
		let cards = div()
			.id((id.clone(), "choices"))
			.flex()
			.flex_col()
			.gap(px(8.))
			.role(Role::Group)
			.aria_label(self.title.clone())
			.children(self.children);
		settings_panel(id, self.title, self.icon)
			.theme(theme)
			.children(self.accessory)
			.child(cards)
			.when_some(self.footnote, |panel, footnote| {
				panel.child(
					div()
						.text_size(tokens::text::CAPTION)
						.text_color(theme.palette.secondary_text)
						.child(footnote),
				)
			})
	}
}

/// An outlined caption capsule, for example "Recommended".
#[derive(IntoElement)]
pub struct Tag {
	text: SharedString,
	theme: Option<Theme>,
}

pub fn tag(text: impl Into<SharedString>) -> Tag {
	Tag {
		text: text.into(),
		theme: None,
	}
}

impl RenderOnce for Tag {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let color = theme.palette.secondary_text;
		div()
			.flex_none()
			.px(px(control::BADGE_HORIZONTAL_PADDING))
			.py(px(control::BADGE_VERTICAL_PADDING))
			.rounded_full()
			.border_1()
			.font_family(theme.sans())
			.border_color(scale_alpha(color, 0.5))
			.text_size(tokens::text::CAPTION)
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(color)
			.child(self.text)
	}
}

/// Where a step is relative to the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
	Done,
	Current,
	Upcoming,
}

/// The setup progress rail: a caption, the steps, and an optional footer.
#[derive(IntoElement)]
pub struct StepRail {
	header: SharedString,
	footer: Option<AnyElement>,
	children: SmallVec<[AnyElement; 6]>,
	theme: Option<Theme>,
}

/// A progress rail under the caption `header`, for example "SETUP ASSISTANT". Add steps with
/// `.child(..)`.
pub fn step_rail(header: impl Into<SharedString>) -> StepRail {
	StepRail {
		header: header.into(),
		footer: None,
		children: SmallVec::new(),
		theme: None,
	}
}

impl StepRail {
	/// An element at the bottom of the rail, for example the version.
	pub fn footer(mut self, footer: impl IntoElement) -> Self {
		self.footer = Some(footer.into_any_element());
		self
	}
}

impl ParentElement for StepRail {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for StepRail {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		div()
			.font_family(theme.sans())
			.h_full()
			.w(px(205.))
			.flex()
			.flex_col()
			.bg(theme.palette.navigation_rail)
			.border_r_1()
			.border_color(theme.palette.hairline)
			.child(
				div()
					.px(px(18.))
					.pt(px(22.))
					.pb(px(14.))
					.text_size(tokens::text::CAPTION)
					.font_weight(FontWeight::SEMIBOLD)
					.letter_spacing(px(1.4))
					.text_color(theme.palette.secondary_text)
					.child(self.header),
			)
			.child(
				div()
					.flex()
					.flex_col()
					.gap(px(4.))
					.px(px(10.))
					.children(self.children),
			)
			.child(div().flex_1())
			.when_some(self.footer, |rail, footer| {
				rail.child(div().p(px(18.)).child(footer))
			})
	}
}

/// One step of a [`step_rail`].
#[derive(IntoElement)]
pub struct Step {
	title: SharedString,
	icon: IconRef,
	state: StepState,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

pub fn step(title: impl Into<SharedString>, icon: impl Into<IconRef>, state: StepState) -> Step {
	Step {
		title: title.into(),
		icon: icon.into(),
		state,
		theme: None,
		on_click: None,
	}
}

impl Step {
	/// Only a done step responds, so the player can go back but not skip ahead.
	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Step {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let id = ElementId::Name(self.title.clone());
		let row = ClickableRow::new(&id, &theme, window, cx);
		let current = self.state == StepState::Current;
		let foreground: Rgba = match self.state {
			StepState::Done => theme.palette.primary_text,
			StepState::Current => tokens::contrast_tint(theme.accent),
			StepState::Upcoming => theme.palette.secondary_text,
		};
		let icon = match self.state {
			StepState::Done => Icon::Checkmark.into(),
			_ => self.icon,
		};
		let on_click = self.on_click.filter(|_| self.state == StepState::Done);
		let clickable = on_click.is_some();
		let item = div()
			.id(id)
			.relative()
			.font_family(theme.sans())
			.min_h(px(38.))
			.px(px(10.))
			.py(px(8.))
			.flex()
			.flex_row()
			.items_center()
			.gap(px(8.))
			.rounded(theme.style.radii.row)
			.rounded_smoothing_ios()
			.text_size(tokens::text::BODY)
			.font_weight(if current {
				FontWeight::SEMIBOLD
			} else {
				FontWeight::NORMAL
			})
			.text_color(foreground)
			.bg(row.fill(&theme, if current { 1. } else { 0. }))
			.role(if clickable { Role::Button } else { Role::ListItem })
			.aria_label(self.title.clone())
			.aria_selected(current);
		// Only a done step responds, so it alone is hovered, focusable, and clickable.
		let item = if clickable {
			row.wire(item, FocusShape::Rounded(theme.style.radii.row), &theme, window)
		} else {
			item
		};
		item.when(current, |item| {
			item.child(
				div()
					.absolute()
					.left(px(0.))
					.top(px(11.))
					.w(px(3.))
					.h(px(16.))
					.rounded_full()
					.bg(theme.accent),
			)
		})
		.child(
			div()
				.w(px(17.))
				.flex()
				.justify_center()
				.child(icon.render(px(14.), foreground)),
		)
		.child(self.title)
		.when_some(on_click, |item, on_click| item.on_click(on_click))
	}
}

theme_override!(ChoiceCard, ChoiceGroup, Tag, StepRail, Step);
