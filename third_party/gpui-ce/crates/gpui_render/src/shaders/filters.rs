#[wgsl_rs::wgsl]
pub mod surface {
    use super::super::common::*;
    use wgsl_rs::std::*;

    #[repr(C)]
    #[derive(Clone, Copy, Wgsl)]
    pub struct SurfaceUniforms {
        pub bounds: Bounds,
        pub content_mask: ContentMask,
        pub color_format: SurfaceColorFormat,
        pub opacity: f32,
        pub padding0: u32,
        pub padding1: u32,
        pub padding2: u32,
        pub padding3: u32,
        pub padding4: u32,
        pub padding5: u32,
    }
    uniform!(group(1), binding(0), SURFACE_LOCALS: SurfaceUniforms);
    texture!(group(1), binding(1), SURFACE_TEXTURE: Texture2D<f32>);
    texture!(group(1), binding(2), SURFACE_CHROMA_TEXTURE: Texture2D<f32>);
    sampler!(group(1), binding(3), SURFACE_SAMPLER: Sampler);

    pub const YCBCR_TO_LINEAR_RGB: Mat4x4f = mat4x4f(
        vec4f(1.0000, 1.0000, 1.0000, 0.0),
        vec4f(0.0000, -0.3441, 1.7720, 0.0),
        vec4f(1.4020, -0.7141, 0.0000, 0.0),
        vec4f(-0.7010, 0.5291, -0.8860, 1.0),
    );

    pub fn sample_yuv_surface(texture_position: Vec2f) -> Vec4f {
        let luma = texture_sample_level(SURFACE_TEXTURE, SURFACE_SAMPLER, texture_position, 0.0).x;
        let chroma = texture_sample_level(
            SURFACE_CHROMA_TEXTURE,
            SURFACE_SAMPLER,
            texture_position,
            0.0,
        )
        .xy();
        YCBCR_TO_LINEAR_RGB * vec4f(luma, chroma.x, chroma.y, 1.0)
    }

    #[derive(Wgsl)]
    pub struct SurfaceVarying {
        #[builtin(position)]
        pub position: Vec4f,
        #[location(0)]
        pub texture_position: Vec2f,
        #[location(3)]
        pub clip_distances: Vec4f,
    }

    #[vertex]
    pub fn vertex_surface(#[builtin(vertex_index)] vertex_id: u32) -> SurfaceVarying {
        let vertex = rectangle_vertex(vertex_id, get!(SURFACE_LOCALS).bounds);
        SurfaceVarying {
            position: vertex.clip_position,
            texture_position: vertex.unit_position,
            clip_distances: clip_distances(
                vertex.viewport_position,
                get!(SURFACE_LOCALS).content_mask.bounds,
            ),
        }
    }

    #[fragment]
    pub fn fragment_surface(input: SurfaceVarying) -> Vec4f {
        if is_clipped(input.clip_distances) {
            return transparent();
        }
        let locals = get!(SURFACE_LOCALS);
        let fade = ContentMask::alpha(locals.content_mask, input.position.xy());
        if locals.color_format == SurfaceColorFormat::Yuv {
            return sample_yuv_surface(input.texture_position) * locals.opacity * fade;
        }
        texture_sample_level(
            SURFACE_TEXTURE,
            SURFACE_SAMPLER,
            input.texture_position,
            0.0,
        ) * locals.opacity
            * fade
    }
}

#[wgsl_rs::wgsl]
pub mod blur {
    use super::super::common::*;
    use super::super::corner_smoothing::*;
    use wgsl_rs::std::*;

    #[repr(C)]
    #[derive(Clone, Copy, Wgsl)]
    pub struct BlurUniforms {
        pub bounds: Bounds,
        pub content_mask: Bounds,
        pub corner_radii: Corners,
        pub direction: Vec2f,
        pub standard_deviation: f32,
        pub opacity: f32,
        pub sample_count: u32,
        pub sample_step: f32,
        pub composite_clip: BlurCompositeClip,
        pub downsample_mode: DownsampleMode,
        pub source_size: Vec2f,
        pub target_size: Vec2f,
        pub corner_smoothing: f32,
        pub padding0: u32,
        pub padding1: u32,
        pub padding2: u32,
        /// Straight-alpha fill over the refracted backdrop.
        pub glass_fill: Vec4f,
        /// Pointer light color (rgb) and strength (a).
        pub glass_pointer_color: Vec4f,
        /// x: bezel width, y: refraction, z: dispersion, w: 1 when glass is enabled.
        pub glass_shape: Vec4f,
        /// x: rim, y: specular, z: light angle, w: saturation.
        pub glass_light: Vec4f,
        /// xy: pointer position in target pixels, z: pointer radius (0 hides it), w: brightness.
        pub glass_pointer: Vec4f,
        /// x: group shape count (0 means the single shape in `bounds`), y: smooth-min spacing.
        pub glass_group: Vec4f,
        /// Group shapes: xy origin, zw size, in target pixels.
        pub glass_rects: [Vec4f; 8],
        /// Group shapes: corner radii as top-left, top-right, bottom-right, bottom-left.
        pub glass_radii: [Vec4f; 8],
        /// Straight-alpha outline colour of the combined group shape (width in `glass_group.z`).
        pub glass_border: Vec4f,
    }
    uniform!(group(1), binding(0), BLUR_LOCALS: BlurUniforms);
    texture!(group(1), binding(1), BLUR_TEXTURE: Texture2D<f32>);
    sampler!(group(1), binding(2), BLUR_SAMPLER: Sampler);

    #[derive(Clone, Copy, Wgsl)]
    pub struct BlurCompositeVertexData {
        pub position: Vec4f,
        pub texture_coordinates: Vec2f,
        pub clip_distances: Vec4f,
    }

    pub fn prepare_blur_composite_vertex(vertex_id: u32) -> BlurCompositeVertexData {
        let vertex = rectangle_vertex(vertex_id, get!(BLUR_LOCALS).bounds);
        BlurCompositeVertexData {
            position: vertex.clip_position,
            texture_coordinates: vertex.unit_position,
            clip_distances: clip_distances(
                vertex.viewport_position,
                get!(BLUR_LOCALS).content_mask,
            ),
        }
    }

    /// Straight-alpha sample of the blurred backdrop at a target-space position.
    pub fn glass_backdrop(position: Vec2f) -> Vec3f {
        let sample = texture_sample_level(
            BLUR_TEXTURE,
            BLUR_SAMPLER,
            position / get!(BLUR_LOCALS).target_size,
            0.0,
        );
        let alpha = max(sample.w, MIN_NORMALIZED_WEIGHT);
        select(
            sample.rgb(),
            sample.rgb() / alpha,
            is_enabled(get!(GLOBALS).premultiplied_alpha),
        )
    }

    /// Smooth minimum of two signed distances with blend distance `spacing`.
    pub fn glass_smooth_min(a: f32, b: f32, spacing: f32) -> f32 {
        let k = max(spacing, 0.0001);
        let h = max(k - abs(a - b), 0.0) / k;
        min(a, b) - h * h * k * 0.25
    }

    /// Combined signed distance of the glass group shapes.
    pub fn glass_group_distance(position: Vec2f) -> f32 {
        let locals = get!(BLUR_LOCALS);
        let count = locals.glass_group.x as u32;
        let spacing = locals.glass_group.y;
        let mut distance = 1.0e9;
        let mut index = 0u32;
        while index < count {
            let rect = locals.glass_rects[index as usize];
            let radii = locals.glass_radii[index as usize];
            let shape = rounded_rectangle_signed_distance(
                position,
                Bounds {
                    origin: rect.xy(),
                    size: rect.zw(),
                },
                Corners {
                    top_left: radii.x,
                    top_right: radii.y,
                    bottom_right: radii.z,
                    bottom_left: radii.w,
                },
            );
            distance = glass_smooth_min(distance, shape, spacing);
            index += 1u32;
        }
        distance
    }

    /// Signed distance of the composited shape: the group if there is one, else the rounded
    /// rectangle in `bounds`.
    pub fn glass_distance(position: Vec2f) -> f32 {
        if get!(BLUR_LOCALS).glass_group.x > 0.5 {
            return glass_group_distance(position);
        }
        rounded_rectangle_signed_distance(
            position,
            get!(BLUR_LOCALS).bounds,
            get!(BLUR_LOCALS).corner_radii,
        )
    }

    /// Refractive glass: the bezel bends the backdrop along the shape normal, red and blue
    /// separate slightly, a thin Fresnel rim and a specular highlight light the edge, and an
    /// optional pointer light glows inside.
    pub fn glass_composite_color(position: Vec2f, coverage: f32) -> Vec4f {
        let locals = get!(BLUR_LOCALS);
        let bezel = max(locals.glass_shape.x, 1.0);
        let refraction = locals.glass_shape.y;
        let dispersion = locals.glass_shape.z;
        let distance = glass_distance(position);
        let gradient = vec2f(
            glass_distance(position + vec2f(1.0, 0.0)) - glass_distance(position - vec2f(1.0, 0.0)),
            glass_distance(position + vec2f(0.0, 1.0)) - glass_distance(position - vec2f(0.0, 1.0)),
        );
        let normal = gradient / max(length(gradient), MIN_NORMALIZED_WEIGHT);
        // 0 on the outer edge, 1 where the bezel meets the flat face.
        let depth = saturate(-distance / bezel);
        let bend = pow(1.0 - depth, 2.4);
        let offset = normal * (refraction * bend);

        let base = glass_backdrop(position + offset);
        let red = glass_backdrop(position + offset * (1.0 + dispersion)).x;
        let blue = glass_backdrop(position + offset * (1.0 - dispersion)).z;
        let mut color = vec3f(red, base.y, blue);

        let luma = dot(color, REC_601_LUMA_WEIGHTS);
        let saturation = locals.glass_light.w;
        color = mix(
            vec3f(luma, luma, luma),
            color,
            vec3f(saturation, saturation, saturation),
        ) * locals.glass_pointer.w;
        let fill_alpha = locals.glass_fill.w;
        color = mix(
            color,
            locals.glass_fill.rgb(),
            vec3f(fill_alpha, fill_alpha, fill_alpha),
        );

        let light = vec2f(cos(locals.glass_light.z), sin(locals.glass_light.z));
        let facing = dot(normal, light);
        // A one-pixel Fresnel rim all around, brighter where the edge faces the light.
        let rim_band = 1.0 - smoothstep(0.0, 1.6, -distance);
        let rim = rim_band * locals.glass_light.x * (0.45 + 0.55 * abs(facing));
        // The bezel catches the light on the lit side and, fainter, on the opposite side.
        let highlight = pow(max(facing, 0.0), 3.0) + 0.35 * pow(max(-facing, 0.0), 3.0);
        let specular = locals.glass_light.y * highlight * pow(1.0 - depth, 1.5);
        color = color + vec3f(rim + specular, rim + specular, rim + specular);

        // One outline along the combined shape, inside its edge.
        let border_alpha = locals.glass_border.w
            * saturate(distance + locals.glass_group.z + 0.5)
            * select(0.0, 1.0, locals.glass_group.z > 0.0);
        color = mix(
            color,
            locals.glass_border.rgb(),
            vec3f(border_alpha, border_alpha, border_alpha),
        );

        if locals.glass_pointer.z > 0.0 {
            let reach = length(position - locals.glass_pointer.xy()) / locals.glass_pointer.z;
            let glow = pow(1.0 - smoothstep(0.0, 1.0, reach), 2.0);
            color =
                color + locals.glass_pointer_color.rgb() * (locals.glass_pointer_color.w * glow);
        }

        // The composite pipeline blends premultiplied color, like the plain blur path.
        let alpha = coverage * locals.opacity;
        vec4f(
            saturate(color.x) * alpha,
            saturate(color.y) * alpha,
            saturate(color.z) * alpha,
            alpha,
        )
    }

    pub fn blur_composite_color(position: Vec2f, coverage: f32) -> Vec4f {
        if get!(BLUR_LOCALS).glass_shape.w > 0.5 {
            return glass_composite_color(position, coverage);
        }
        let blurred = texture_sample_level(
            BLUR_TEXTURE,
            BLUR_SAMPLER,
            position / get!(BLUR_LOCALS).target_size,
            0.0,
        );
        let factor = coverage * get!(BLUR_LOCALS).opacity;
        vec4f(
            blurred.x * factor,
            blurred.y * factor,
            blurred.z * factor,
            blurred.w * factor,
        )
    }

    #[derive(Wgsl)]
    pub struct BlurVarying {
        #[builtin(position)]
        pub position: Vec4f,
        #[location(0)]
        pub texture_coordinates: Vec2f,
        #[location(3)]
        pub clip_distances: Vec4f,
    }

    #[vertex]
    pub fn vertex_blur_fullscreen(#[builtin(vertex_index)] vertex_id: u32) -> BlurVarying {
        let vertex = fullscreen_vertex(vertex_id);
        BlurVarying {
            position: vertex.clip_position,
            texture_coordinates: vertex.texture_coordinates,
            clip_distances: unclipped_distances(),
        }
    }

    #[fragment]
    pub fn fragment_blur_downsample(input: BlurVarying) -> Vec4f {
        if get!(BLUR_LOCALS).downsample_mode == DownsampleMode::HalfResolution {
            let destination = floor(input.position.xy());
            let source_coordinates = min(
                (destination + 0.5) / get!(BLUR_LOCALS).target_size,
                (get!(BLUR_LOCALS).source_size - 0.5) / get!(BLUR_LOCALS).source_size,
            );
            return texture_sample_level(BLUR_TEXTURE, BLUR_SAMPLER, source_coordinates, 0.0);
        }
        texture_sample_level(BLUR_TEXTURE, BLUR_SAMPLER, input.texture_coordinates, 0.0)
    }

    pub fn sample_blur_texture(texture_coordinates: Vec2f) -> Vec4f {
        texture_sample_level(BLUR_TEXTURE, BLUR_SAMPLER, texture_coordinates, 0.0)
    }

    pub fn gaussian_blur(texture_coordinates: Vec2f) -> Vec4f {
        let uniforms = get!(BLUR_LOCALS);
        let center_weight = gaussian(0.0, uniforms.standard_deviation);
        let mut weighted_color = sample_blur_texture(texture_coordinates) * center_weight;
        let mut weight_sum = center_weight;
        let mut sample_index = 1u32;

        // Taps one texel apart share one bilinear fetch at their weighted midpoint, which gives
        // the same sum with half the fetches. Taps spread wider (large radii) are fetched alone.
        while sample_index <= uniforms.sample_count {
            let distance = sample_index as f32 * uniforms.sample_step;
            let mut weight = gaussian(distance, uniforms.standard_deviation);
            let mut offset = distance;
            let mut taps = 1u32;
            if uniforms.sample_step <= 1.0 && sample_index < uniforms.sample_count {
                let next_distance = distance + uniforms.sample_step;
                let next_weight = gaussian(next_distance, uniforms.standard_deviation);
                offset = (distance * weight + next_distance * next_weight)
                    / max(weight + next_weight, MIN_NORMALIZED_WEIGHT);
                weight = weight + next_weight;
                taps = 2u32;
            }
            let coordinate_offset = uniforms.direction * offset;
            let symmetric_pair = sample_blur_texture(texture_coordinates - coordinate_offset)
                + sample_blur_texture(texture_coordinates + coordinate_offset);
            weighted_color = weighted_color + symmetric_pair * weight;
            weight_sum += 2.0 * weight;
            sample_index += taps;
        }
        weighted_color / max(weight_sum, MIN_NORMALIZED_WEIGHT)
    }

    #[fragment]
    pub fn fragment_blur(input: BlurVarying) -> Vec4f {
        gaussian_blur(input.texture_coordinates)
    }

    #[vertex]
    pub fn vertex_blur_composite(#[builtin(vertex_index)] vertex_id: u32) -> BlurVarying {
        let vertex = prepare_blur_composite_vertex(vertex_id);
        BlurVarying {
            position: vertex.position,
            texture_coordinates: vertex.texture_coordinates,
            clip_distances: vertex.clip_distances,
        }
    }

    #[fragment]
    pub fn fragment_blur_composite(input: BlurVarying) -> Vec4f {
        if is_clipped(input.clip_distances) {
            return transparent();
        }
        let coverage = select(
            1.0,
            antialiased_coverage(glass_distance(input.position.xy())),
            get!(BLUR_LOCALS).composite_clip == BlurCompositeClip::RoundedBounds,
        );
        blur_composite_color(input.position.xy(), coverage)
    }

    #[derive(Wgsl)]
    pub struct SmoothedBlurVarying {
        #[builtin(position)]
        pub position: Vec4f,
        #[location(0)]
        pub texture_coordinates: Vec2f,
        #[location(1)]
        #[interpolate(flat)]
        pub horizontal_corner_reaches: Vec4f,
        #[location(2)]
        #[interpolate(flat)]
        pub vertical_corner_reaches: Vec4f,
        #[location(3)]
        pub clip_distances: Vec4f,
        #[location(4)]
        #[interpolate(flat)]
        pub smoothing_factors: Vec4f,
        #[location(5)]
        #[interpolate(flat)]
        pub superellipse_power: f32,
    }

    #[vertex]
    pub fn vertex_smoothed_blur_composite(
        #[builtin(vertex_index)] vertex_id: u32,
    ) -> SmoothedBlurVarying {
        let vertex = prepare_blur_composite_vertex(vertex_id);
        let prepared = prepare_corners(
            get!(BLUR_LOCALS).bounds.size,
            get!(BLUR_LOCALS).corner_radii,
            get!(BLUR_LOCALS).corner_smoothing,
            true,
        );
        SmoothedBlurVarying {
            position: vertex.position,
            texture_coordinates: vertex.texture_coordinates,
            horizontal_corner_reaches: prepared.horizontal_reaches,
            vertical_corner_reaches: prepared.vertical_reaches,
            clip_distances: vertex.clip_distances,
            smoothing_factors: prepared.smoothing_factors,
            superellipse_power: prepared.superellipse_power,
        }
    }

    #[fragment]
    pub fn fragment_smoothed_blur_composite(input: SmoothedBlurVarying) -> Vec4f {
        if is_clipped(input.clip_distances) {
            return transparent();
        }
        let coverage = select(
            1.0,
            antialiased_coverage(prepared_corner_signed_distance(
                input.position.xy(),
                get!(BLUR_LOCALS).bounds,
                get!(BLUR_LOCALS).corner_radii,
                get!(BLUR_LOCALS).corner_smoothing,
                PreparedCorners {
                    horizontal_reaches: input.horizontal_corner_reaches,
                    vertical_reaches: input.vertical_corner_reaches,
                    smoothing_factors: input.smoothing_factors,
                    superellipse_power: input.superellipse_power,
                },
            )),
            get!(BLUR_LOCALS).composite_clip == BlurCompositeClip::RoundedBounds,
        );
        blur_composite_color(input.position.xy(), coverage)
    }
}
