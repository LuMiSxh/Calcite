# New gpui-ce API in `gpui-ce-glass.patch`

Everything below is in the `gpui` crate (re-exported at the crate root) and needs no change in
the backends' public API. All paint methods are paint-phase only.

## 1. Paint transform

```rust
impl Window {
    pub fn with_paint_transform<R>(
        &mut self,
        scale: f32,
        origin: Point<Pixels>,
        f: impl FnOnce(&mut Window) -> R,
    ) -> R;
    pub fn paint_transform(&self) -> PaintTransform; // composed transform, for inspection
}

pub struct PaintTransform { pub scale: f32, pub translation: Point<Pixels> } // p' = scale*p + translation
impl PaintTransform { IDENTITY, about(scale, origin), then(inner), apply(point), is_identity() }
```

Semantics

- Everything painted inside `f` is scaled uniformly by `scale` about `origin`. `origin` is a point
  in the coordinates painted inside, i.e. normal window-space logical pixels (for a control, pass
  `bounds.center()`). No rotation.
- Nested calls compose; the inner transform applies first (`outer.then(inner)`).
- Layout, hitboxes and hit-testing are not affected. Content masks are NOT transformed (they stay
  a screen-space clip), so a control scaled above 1 is clipped by its ancestors' masks as before.
- Transformed at paint time: quads (bounds, corner radii, border widths, dash length and gap),
  drop and inset shadows (bounds, radii, blur radius), underlines and strikethroughs, paths
  (vertices), glyph sprites (monochrome, subpixel, color), SVG sprites, image sprites, surfaces,
  `paint_backdrop_filter` and `with_filter_layer` (bounds, radii, blur radius, glass bezel,
  refraction, pointer light) and `paint_glass_group`.
- Glyphs, SVGs and images keep their original raster (same atlas tile, same cache key); only the
  sprite bounds scale, so the GPU resamples them. This means no atlas churn while animating, at
  the cost of slightly soft text far from scale 1. Subpixel (LCD) text is scaled too.
- Identity (scale 1, no translation) takes the exact old path.

From Calcite: wrap the control's whole paint (glass, fill, border, label, icon) in
`window.with_paint_transform(frame.scale, bounds.center(), |window| { ... })` and drop
`ControlFrame::surface_insets`, so text and icons scale with the surface. Keep hit-testing on the
untransformed bounds.

## 2. Glass groups (morphing glass)

```rust
pub const MAX_GLASS_SHAPES: usize = 8;

pub struct GlassShape { pub bounds: Bounds<Pixels>, pub corner_radii: Corners<Pixels> }

pub struct GlassGroup {
    pub shapes: SmallVec<[GlassShape; MAX_GLASS_SHAPES]>,
    pub spacing: Pixels,      // smooth-min blend distance
    pub glass: GlassFilter,   // the material, same struct as Filter::Glass
    pub border: [f32; 4],     // straight-alpha outline colour of the combined shape
    pub border_width: Pixels, // outline width, inside the edge; 0 draws none
}
impl GlassGroup {
    pub fn new(glass: GlassFilter) -> Self;
    pub fn shape(self, bounds: Bounds<Pixels>, corner_radii: Corners<Pixels>) -> Self;
    pub fn spacing(self, spacing: Pixels) -> Self;
    pub fn border(self, color: [f32; 4], width: Pixels) -> Self; // one outline for the union
}

impl Window {
    pub fn paint_glass_group(&mut self, group: &GlassGroup);
}
```

Semantics

- One glass pass for the whole group. The signed distance fields of all shapes are combined with a
  polynomial smooth minimum (`k = spacing`, `h = max(k - |a-b|, 0)/k`, `min(a,b) - h*h*k/4`,
  folded over the shapes in order). Shapes closer than about `spacing` visibly fuse; farther
  apart they are exactly the plain union. Coverage (the clip), refraction normal, bezel depth, rim
  and specular are all computed once from the combined field. `spacing = 0` gives a hard union.
- Shape coordinates are the same logical window coordinates you would give `paint_backdrop_filter`
  and follow the active paint transform.
- At most `MAX_GLASS_SHAPES` (8) shapes are used; extra shapes are ignored. Empty shapes are
  skipped; no shapes paints nothing.
- Group shapes are plain rounded rectangles: corner smoothing is not applied.
- A group with one shape renders like `paint_backdrop_filter(bounds, radii, &[Filter::Glass(..)])`
  (same SDF, same coverage); the existing single-shape path (`Filter::Glass`) is unchanged and
  still the cheapest option for one control.
- Animate a morph by changing shape bounds/radii per frame (e.g. interpolate a selection pill
  towards its neighbour while `spacing` stays around 20-40 px).
- Cost: the fragment shader loops over the shapes five times per pixel (distance plus the
  4-tap gradient), so keep groups small.

Scene/renderer plumbing (for reference): `BackdropFilter` gained `glass_group:
Option<Box<ScaledGlassGroup>>`; `BackdropFilter::glass()` now returns `Option<ScaledGlass>`
(`{ filter: ScaledGlassFilter, group: ScaledGlassGroup }`); `BlurUniforms` grew by 272 bytes
(`glass_group`, `glass_rects[8]`, `glass_radii[8]`, `glass_border`, 480 bytes total). The shader is written once
in `gpui_render/src/shaders/filters.rs` and compiled to MSL, WGSL and HLSL, so Metal, wgpu
(Linux) and Direct3D share it.
