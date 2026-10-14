// SPDX-License-Identifier: MPL-2.0

//! Window chrome: the launcher's hidden title bar on macOS, Windows, and Linux.
//!
//! The content fills the whole window, and the app draws its own title bar over it:
//!
//! - **macOS:** the system traffic lights stay. The app owns the title bar drag, so AppKit
//!   does not delay clicks on controls in the bar.
//! - **Windows:** the app draws the same round caption buttons as Linux, with a red close. The
//!   system hit-tests them and the drag region, so Snap Layouts, the window menu, and
//!   double-click maximize work. The system keeps the frame.
//! - **Linux:** the app asks for client-side decorations. With them it draws the caption
//!   buttons, the shadow, the rounded frame, and the resize edges. Without a compositor the
//!   window manager draws its own frame, and the bar only holds the app's controls.
//!
//! ```ignore
//! cx.open_window(
//!     WindowOptions { window_min_size: Some(size(px(880.), px(560.))), ..chrome::window_options("App", bounds) },
//!     |window, cx| cx.new(|cx| App::new(window, cx)),
//! );
//!
//! // In the root view:
//! window_frame(div().size_full().child(content).child(title_bar("title-bar").child(settings)))
//! ```
//!
//! GPUI clips to rectangles, so the frame cannot round the corners of full-bleed content.
//! Give full-bleed images [`corner_radius`] on Linux.

use std::rc::Rc;

use gpui::{
	AnyElement, App, Bounds, BoxShadow, CursorStyle, Decorations, ElementId, HitboxBehavior, IntoElement,
	MouseButton, ParentElement, Pixels, Point, ResizeEdge, Rgba, Role, SharedString, Size, Styled, Tiling,
	TitlebarOptions, Window, WindowBounds, WindowControlArea, WindowDecorations, WindowOptions, canvas, div,
	prelude::*, px,
};

use crate::{
	icons::Icon,
	interaction::{ControlInteraction, ControlState, Feedback},
	theme::{ActiveTheme, Theme},
	tokens::{black, mix, rgba, white},
};

/// The height of the title bar: room for a 44 pt hit target and the top padding.
const BAR_HEIGHT: Pixels = px(52.);
/// The space the macOS traffic lights take at the leading edge.
const TRAFFIC_LIGHTS: Pixels = px(78.);
const BAR_PADDING: Pixels = px(14.);
const CAPTION_GLYPH: Pixels = px(10.);
/// Caption buttons, after GNOME's header bar. Linux and Windows draw the same ones.
const ROUND_CAPTION: Pixels = px(24.);
const ROUND_CAPTION_GAP: Pixels = px(10.);
/// The invisible margin that holds the client-side shadow and the resize edges.
const SHADOW: Pixels = px(12.);
const RADIUS: Pixels = px(10.);

/// The chrome of one platform. [`ChromeStyle::native`] is the one this build runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeStyle {
	MacOs,
	Windows,
	Linux,
}

impl ChromeStyle {
	pub const fn native() -> Self {
		if cfg!(target_os = "macos") {
			Self::MacOs
		} else if cfg!(target_os = "windows") {
			Self::Windows
		} else {
			Self::Linux
		}
	}
}

/// Window options for a window that draws its own title bar on every platform. Override
/// other fields with struct update syntax.
pub fn window_options(title: impl Into<SharedString>, bounds: Bounds<Pixels>) -> WindowOptions {
	WindowOptions {
		window_bounds: Some(WindowBounds::Windowed(bounds)),
		titlebar: Some(TitlebarOptions {
			title: Some(title.into()),
			appears_transparent: true,
			traffic_light_position: None,
		}),
		app_owns_titlebar_drag: true,
		window_decorations: Some(WindowDecorations::Client),
		// The client-side shadow is drawn outside the frame, so the margin must show through.
		#[cfg(any(target_os = "linux", target_os = "freebsd"))]
		linux_window_background: gpui::LinuxWindowBackground::Transparent,
		..Default::default()
	}
}

/// The tiling of a client-decorated window, or `None` when the system draws the frame.
fn client_tiling(window: &Window) -> Option<Tiling> {
	match window.window_decorations() {
		Decorations::Client { tiling } if ChromeStyle::native() == ChromeStyle::Linux => Some(tiling),
		_ => None,
	}
}

/// The corner radius of the window frame. It is zero unless the app draws the frame.
pub fn corner_radius(window: &Window) -> Pixels {
	match client_tiling(window) {
		Some(tiling) if !tiling.is_tiled() => RADIUS,
		_ => px(0.),
	}
}

/// The root of a window with [`window_options`]. On Linux with client-side decorations it
/// draws the shadow, the frame, and the resize edges; elsewhere it only fills the window.
#[derive(IntoElement)]
pub struct WindowFrame {
	content: AnyElement,
}

pub fn window_frame(content: impl IntoElement) -> WindowFrame {
	WindowFrame {
		content: content.into_any_element(),
	}
}

impl RenderOnce for WindowFrame {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let Some(tiling) = client_tiling(window) else {
			return div().size_full().child(self.content).into_any_element();
		};
		let inset = if tiling.is_tiled() { px(0.) } else { SHADOW };
		// The platform writes the frame extents on every call; skip unchanged values.
		if window.client_inset() != Some(inset) {
			window.set_client_inset(inset);
		}
		let theme = cx.theme();
		// The cursor is set while painting, so a repaint is only needed when the hovered edge
		// changes. Refreshing on every move would also invalidate every cached view.
		let hovered_edge = window.use_keyed_state("calcite-frame-edge", cx, |_, _| None::<ResizeEdge>);
		let radius = |tiled_a: bool, tiled_b: bool| if tiled_a || tiled_b { px(0.) } else { RADIUS };
		let track_edge = {
			let hovered_edge = hovered_edge.clone();
			move |position: Point<Pixels>, window: &mut Window, cx: &mut App| {
				let edge = resize_edge(position, window.viewport_size(), tiling);
				if hovered_edge.read(cx) != &edge {
					hovered_edge.update(cx, |current, _| *current = edge);
					window.refresh();
				}
			}
		};
		let track_edge_in_frame = track_edge.clone();
		let frame = div()
			.size_full()
			.overflow_hidden()
			.bg(theme.palette.backdrop)
			.rounded_tl(radius(tiling.top, tiling.left))
			.rounded_tr(radius(tiling.top, tiling.right))
			.rounded_bl(radius(tiling.bottom, tiling.left))
			.rounded_br(radius(tiling.bottom, tiling.right))
			.border_color(theme.palette.hairline)
			.when(!tiling.top, |frame| frame.border_t_1())
			.when(!tiling.bottom, |frame| frame.border_b_1())
			.when(!tiling.left, |frame| frame.border_l_1())
			.when(!tiling.right, |frame| frame.border_r_1())
			.when(!tiling.is_tiled(), |frame| {
				frame.shadow(vec![
					BoxShadow::new(px(0.), px(2.), black(0.42)).blur_radius(SHADOW / 2.),
				])
			})
			// The frame takes the pointer, so the margin alone resizes.
			.on_mouse_move(move |event, window, cx| {
				track_edge_in_frame(event.position, window, cx);
				cx.stop_propagation();
			})
			.child(self.content);
		div()
			.id("calcite-window-frame")
			.font_family(theme.sans())
			.size_full()
			.when(!tiling.top, |root| root.pt(inset))
			.when(!tiling.bottom, |root| root.pb(inset))
			.when(!tiling.left, |root| root.pl(inset))
			.when(!tiling.right, |root| root.pr(inset))
			.child(
				canvas(
					|_, window, _| {
						window.insert_hitbox(
							Bounds::new(Point::default(), window.viewport_size()),
							HitboxBehavior::Normal,
						)
					},
					move |_, hitbox, window, _| {
						if let Some(edge) =
							resize_edge(window.mouse_position(), window.viewport_size(), tiling)
						{
							window.set_cursor_style(CursorStyle::from(edge), &hitbox);
						}
					},
				)
				.absolute()
				.size_full(),
			)
			.on_mouse_move(move |event, window, cx| track_edge(event.position, window, cx))
			.on_mouse_down(MouseButton::Left, move |event, window, _| {
				if let Some(edge) = resize_edge(event.position, window.viewport_size(), tiling) {
					window.start_window_resize(edge);
				}
			})
			.child(frame)
			.into_any_element()
	}
}

/// The edge under `position` in the shadow margin. Corners reach further along each edge.
fn resize_edge(position: Point<Pixels>, window: Size<Pixels>, tiling: Tiling) -> Option<ResizeEdge> {
	let corner = SHADOW * 2.;
	let top = !tiling.top && position.y < SHADOW;
	let bottom = !tiling.bottom && position.y > window.height - SHADOW;
	let left = !tiling.left && position.x < SHADOW;
	let right = !tiling.right && position.x > window.width - SHADOW;
	let near_top = !tiling.top && position.y < corner;
	let near_bottom = !tiling.bottom && position.y > window.height - corner;
	let near_left = !tiling.left && position.x < corner;
	let near_right = !tiling.right && position.x > window.width - corner;
	Some(match () {
		_ if (top || left) && near_top && near_left => ResizeEdge::TopLeft,
		_ if (top || right) && near_top && near_right => ResizeEdge::TopRight,
		_ if (bottom || left) && near_bottom && near_left => ResizeEdge::BottomLeft,
		_ if (bottom || right) && near_bottom && near_right => ResizeEdge::BottomRight,
		_ if top => ResizeEdge::Top,
		_ if bottom => ResizeEdge::Bottom,
		_ if left => ResizeEdge::Left,
		_ if right => ResizeEdge::Right,
		_ => return None,
	})
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The title bar over the top of the content: the drag region, the platform's caption
/// buttons, and the app's trailing controls.
#[derive(IntoElement)]
pub struct TitleBar {
	id: ElementId,
	style: ChromeStyle,
	preview: bool,
	children: Vec<AnyElement>,
	on_close: Option<CloseHandler>,
}

pub fn title_bar(id: impl Into<ElementId>) -> TitleBar {
	TitleBar {
		id: id.into(),
		style: ChromeStyle::native(),
		preview: false,
		children: Vec::new(),
		on_close: None,
	}
}

impl TitleBar {
	/// Draws the chrome of `style` as a preview. A preview never moves or controls the window.
	pub fn preview(mut self, style: ChromeStyle) -> Self {
		self.style = style;
		self.preview = true;
		self
	}

	/// Runs instead of closing the window when the app draws the close button (Linux). The
	/// system close on macOS and Windows runs `Window::on_window_should_close` instead.
	pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
		self.on_close = Some(Rc::new(handler));
		self
	}
}

impl ParentElement for TitleBar {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

/// What the bar draws in this window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Controls {
	/// The system draws them: the traffic lights, or a server-side frame.
	System,
	Round,
}

impl RenderOnce for TitleBar {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		let live = !self.preview;
		let controls = match self.style {
			ChromeStyle::MacOs => Controls::System,
			ChromeStyle::Windows => Controls::Round,
			ChromeStyle::Linux if !live || client_tiling(window).is_some() => Controls::Round,
			ChromeStyle::Linux => Controls::System,
		};
		// A server-side frame already moves the window.
		let drags = live && !(self.style == ChromeStyle::Linux && controls == Controls::System);
		let leading = if self.style == ChromeStyle::MacOs {
			TRAFFIC_LIGHTS
		} else {
			BAR_PADDING
		};
		div()
			.id(self.id.clone())
			.absolute()
			.top_0()
			.left_0()
			.right_0()
			.h(BAR_HEIGHT)
			.font_family(theme.sans())
			.child(drag_region(&self.id, self.style, drags, window, cx))
			.child(
				div()
					.absolute()
					.size_full()
					.pl(leading)
					.pr(BAR_PADDING)
					.flex()
					.flex_row()
					.items_center()
					.justify_end()
					.gap(px(8.))
					// Each control blocks the drag region, so pressing it never moves the window.
					.children(
						self.children
							.into_iter()
							.map(|child| div().occlude().child(child)),
					)
					.when(controls == Controls::Round, |row| {
						row.child(round_captions(
							&self.id,
							live,
							self.on_close,
							&theme,
							self.style,
							window,
							cx,
						))
					}),
			)
	}
}

fn drag_region(
	id: &ElementId,
	style: ChromeStyle,
	drags: bool,
	window: &mut Window,
	cx: &mut App,
) -> AnyElement {
	let region = div().absolute().size_full();
	if !drags {
		return region.into_any_element();
	}
	if style == ChromeStyle::Windows {
		// The system moves the window and handles double clicks and the window menu.
		return region
			.window_control_area(WindowControlArea::Drag)
			.into_any_element();
	}
	// A move starts on the first drag, not on the press, so a double click still arrives.
	let pending = window.use_keyed_state((id.clone(), "calcite-title-drag"), cx, |_, _| false);
	let (press, drag, release) = (pending.clone(), pending.clone(), pending);
	region
		.on_mouse_down(MouseButton::Left, move |event, window, cx| {
			if event.click_count >= 2 {
				if style == ChromeStyle::MacOs {
					window.titlebar_double_click();
				} else {
					window.zoom_window();
				}
			} else {
				press.write(cx, true);
			}
		})
		.on_mouse_move(move |_, window, cx| {
			if *drag.read(cx) {
				drag.write(cx, false);
				window.start_window_move();
			}
		})
		.on_mouse_up(MouseButton::Left, move |_, _, cx| release.write(cx, false))
		.when(style == ChromeStyle::Linux, |region| {
			region.on_mouse_down(MouseButton::Right, |event, window, _| {
				window.show_window_menu(event.position);
			})
		})
		.into_any_element()
}

fn glyph_color(theme: &Theme, active: bool) -> Rgba {
	if active {
		theme.palette.primary_text
	} else {
		theme.palette.secondary_text
	}
}

/// GNOME-style round caption buttons in the trailing controls.
fn round_captions(
	id: &ElementId,
	live: bool,
	on_close: Option<CloseHandler>,
	theme: &Theme,
	style: ChromeStyle,
	window: &mut Window,
	cx: &mut App,
) -> impl IntoElement {
	let active = window.is_window_active();
	let controls = if live {
		window.window_controls()
	} else {
		gpui::WindowControls::default()
	};
	let maximize = if live && window.is_maximized() {
		(Icon::WindowRestore, "Restore")
	} else {
		(Icon::WindowMaximize, "Maximize")
	};
	let mut buttons = Vec::new();
	if controls.minimize {
		buttons.push((Icon::WindowMinimize, "Minimize"));
	}
	if controls.maximize {
		buttons.push(maximize);
	}
	buttons.push((Icon::WindowClose, "Close"));
	div()
		.flex()
		.flex_row()
		.items_center()
		.gap(ROUND_CAPTION_GAP)
		.pl(px(4.))
		.children(buttons.into_iter().map(|(icon, label)| {
			let id = ElementId::from((id.clone(), label));
			let state = ControlState::of(&id, window, cx);
			let frame =
				ControlState::frame(&state, Feedback::PressOnly, true, theme.reduce_motion, window, cx);
			let strength = 0.12 + 0.08 * frame.light + if frame.pressed { 0.06 } else { 0. };
			let (fill, glyph) = if style == ChromeStyle::Windows && matches!(icon, Icon::WindowClose) {
				// Windows closes in the danger color, with a white glyph on hover.
				let danger = theme.palette.danger;
				let shade = if frame.pressed { 0.85 } else { 1. };
				let red = rgba(
					danger.red * shade,
					danger.green * shade,
					danger.blue * shade,
					danger.alpha,
				);
				let glyph = if frame.hovered {
					white(1.)
				} else {
					glyph_color(theme, active)
				};
				(mix(white(strength), red, frame.light), glyph)
			} else {
				(white(strength), glyph_color(theme, active))
			};
			let on_close = on_close.clone();
			div()
				.id(id)
				.size(ROUND_CAPTION)
				.rounded_full()
				.flex()
				.items_center()
				.justify_center()
				.bg(fill)
				.occlude()
				.role(Role::Button)
				.aria_label(label)
				.control_state(&state, false)
				.child(icon.render(CAPTION_GLYPH, glyph))
				.when(live, |button| match style {
					// The system hit-tests these, so Snap Layouts and the close run natively.
					ChromeStyle::Windows => button.window_control_area(match icon {
						Icon::WindowMinimize => WindowControlArea::Min,
						Icon::WindowClose => WindowControlArea::Close,
						_ => WindowControlArea::Max,
					}),
					_ => button.on_click(move |_, window, cx| match icon {
						Icon::WindowMinimize => window.minimize_window(),
						Icon::WindowClose => match &on_close {
							Some(on_close) => on_close(window, cx),
							None => window.remove_window(),
						},
						_ => window.zoom_window(),
					}),
				})
		}))
}

#[cfg(test)]
mod tests {
	use gpui::point;

	use super::*;

	#[test]
	fn resize_edges_follow_the_margin_and_skip_tiled_sides() {
		let window = gpui::size(px(800.), px(600.));
		let free = Tiling::default();
		let at = |x: f32, y: f32| point(px(x), px(y));
		assert_eq!(resize_edge(at(400., 4.), window, free), Some(ResizeEdge::Top));
		assert_eq!(resize_edge(at(4., 4.), window, free), Some(ResizeEdge::TopLeft));
		assert_eq!(resize_edge(at(4., 20.), window, free), Some(ResizeEdge::TopLeft));
		assert_eq!(
			resize_edge(at(796., 596.), window, free),
			Some(ResizeEdge::BottomRight)
		);
		assert_eq!(resize_edge(at(400., 300.), window, free), None);
		let tiled = Tiling {
			top: true,
			..Tiling::default()
		};
		assert_eq!(resize_edge(at(400., 4.), window, tiled), None);
		assert_eq!(resize_edge(at(4., 4.), window, tiled), Some(ResizeEdge::Left));
	}
}
