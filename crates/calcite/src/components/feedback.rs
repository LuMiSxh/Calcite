// SPDX-License-Identifier: MPL-2.0

//! Small feedback elements: [`skeleton`] (`SkeletonValue`), [`badge`], and glass
//! tooltips through [`TooltipExt::glass_tooltip`].

use gpui::{
	AnyView, App, Context, FontWeight, IntoElement, ParentElement, Pixels, Render, SharedString,
	StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
};

use crate::{
	continuous::ContinuousMotion,
	controls::{Tone, theme_override},
	glass::{Elevation, GlassExt},
	motion::{EnterExt, tokens::TOOLTIP},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, white, with_alpha},
};

const SKELETON_BASELINE: f32 = 0.12;
const SKELETON_PEAK: f32 = 0.22;
const SKELETON_CYCLE: f32 = 1.8;

/// A placeholder for a value whose size is already known.
#[derive(IntoElement)]
pub struct Skeleton {
	id: gpui::ElementId,
	width: Option<Pixels>,
	height: Option<Pixels>,
	corner: Option<Pixels>,
	pulses: bool,
	theme: Option<Theme>,
}

/// A placeholder. Leave the size unset to fill a cell the caller constrains.
pub fn skeleton(id: impl Into<gpui::ElementId>) -> Skeleton {
	Skeleton {
		id: id.into(),
		width: None,
		height: None,
		corner: None,
		pulses: false,
		theme: None,
	}
}

impl Skeleton {
	pub fn size(mut self, width: Pixels, height: Pixels) -> Self {
		self.width = Some(width);
		self.height = Some(height);
		self
	}

	pub fn corner(mut self, corner: Pixels) -> Self {
		self.corner = Some(corner);
		self
	}

	/// Pulses gently while decorative motion is allowed.
	pub fn pulses(mut self, pulses: bool) -> Self {
		self.pulses = pulses;
		self
	}
}

impl RenderOnce for Skeleton {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let placeholder = div()
			.id(self.id)
			// Without an explicit corner, a placeholder rounds like a control of the theme.
			.rounded(self.corner.unwrap_or(theme.style.radii.control))
			.bg(white(SKELETON_BASELINE))
			.map(|this| match self.width {
				Some(width) => this.w(width),
				None => this.w_full(),
			})
			.map(|this| match self.height {
				Some(height) => this.h(height),
				None => this.h_full(),
			});
		if self.pulses && !theme.reduce_motion {
			let phase = ContinuousMotion::phase("calcite-skeleton", SKELETON_CYCLE, &theme, window, cx);
			let wave = if phase < 0.5 {
				phase * 2.
			} else {
				(1. - phase) * 2.
			};
			let eased = wave * wave * (3. - 2. * wave);
			placeholder
				.bg(white(
					SKELETON_BASELINE + (SKELETON_PEAK - SKELETON_BASELINE) * eased,
				))
				.into_any_element()
		} else {
			placeholder.into_any_element()
		}
	}
}

/// A small capsule label, for example "Canary" or "New".
#[derive(IntoElement)]
pub struct Badge {
	text: SharedString,
	tone: Tone,
	theme: Option<Theme>,
}

pub fn badge(text: impl Into<SharedString>) -> Badge {
	Badge {
		text: text.into(),
		tone: Tone::Accent,
		theme: None,
	}
}

impl Badge {
	pub fn tone(mut self, tone: Tone) -> Self {
		self.tone = tone;
		self
	}
}

impl RenderOnce for Badge {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let color = match self.tone {
			Tone::Accent => theme.accent,
			Tone::Color(color) => color,
			Tone::Neutral => theme.palette.control,
			Tone::Warning => theme.palette.warning,
			Tone::Danger => theme.palette.danger_text,
		};
		div()
			.font_family(theme.sans())
			.px(px(control::BADGE_HORIZONTAL_PADDING))
			.py(px(control::BADGE_VERTICAL_PADDING))
			.rounded_full()
			.bg(with_alpha(color, 0.16))
			.border_1()
			.border_color(with_alpha(color, 0.4))
			.text_size(tokens::text::CAPTION)
			.font_weight(FontWeight::BOLD)
			.text_color(tokens::contrast_tint(color))
			.child(self.text)
	}
}

/// The view a glass tooltip renders.
pub struct GlassTooltip {
	text: SharedString,
}

impl Render for GlassTooltip {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.font_family(theme.sans())
			.max_w(px(280.))
			.px(px(10.))
			.py(px(6.))
			.rounded(theme.style.radii.control)
			.glass_hud_at(&theme, Elevation::Resting)
			.text_size(tokens::text::FOOTNOTE)
			.text_color(theme.palette.primary_text)
			.child(self.text.clone())
			.enter_with("calcite-tooltip", 0, TOOLTIP, theme.reduce_motion)
	}
}

/// Adds a glass tooltip to any interactive element with an id.
pub trait TooltipExt: StatefulInteractiveElement + Sized {
	fn glass_tooltip(self, text: impl Into<SharedString>) -> Self {
		let text = text.into();
		self.tooltip(move |_, cx| AnyView::from(cx.new(|_| GlassTooltip { text: text.clone() })))
	}
}

impl<E: StatefulInteractiveElement + Sized> TooltipExt for E {}

theme_override!(Skeleton, Badge);
