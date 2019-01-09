use glyph_brush::VariedSection;
use glyph_brush::rusttype::{Font, SharedBytes, Rect, point};
use std::borrow::Cow;

const IDENTITY_MATRIX4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub struct GlyphBrushBuilder<'a, H = glyph_brush::DefaultSectionHasher> {
	inner: glyph_brush::GlyphBrushBuilder<'a, H>,
}

impl<'a> GlyphBrushBuilder<'a> {
    #[inline]
    pub fn using_font_bytes<F: Into<SharedBytes<'a>>>(font: F) -> Self {
		Self::using_font(Font::from_bytes(font).unwrap())
    }

    #[inline]
    pub fn using_fonts_bytes<B, V>(font_data: V) -> Self
    where
        B: Into<SharedBytes<'a>>,
        V: Into<Vec<B>>,
    {
        Self::using_fonts(
            font_data
                .into()
                .into_iter()
                .map(|data| Font::from_bytes(data).unwrap())
                .collect::<Vec<_>>(),
        )
    }

    #[inline]
    pub fn using_font(font_0: Font<'a>) -> Self {
        Self::using_fonts(vec![font_0])
    }

    pub fn using_fonts<V: Into<Vec<Font<'a>>>>(fonts: V) -> Self {
        GlyphBrushBuilder {
            inner: glyph_brush::GlyphBrushBuilder::using_fonts(fonts),
        }
    }
}
impl<'a, H: std::hash::BuildHasher> GlyphBrushBuilder<'a, H> {
    glyph_brush::delegate_glyph_brush_builder_fns!(inner);

    pub fn build<'g>(self, grr: &'g grr::Device) -> GlyphBrush<'a, 'g, H> {
        let vs = grr.create_shader(grr::ShaderStage::Vertex, include_bytes!("shaders/vert.glsl")).unwrap();
        let fs = grr.create_shader(grr::ShaderStage::Fragment, include_bytes!("shaders/frag.glsl")).unwrap();
        let pipeline = grr.create_graphics_pipeline(grr::GraphicsPipelineDesc {
            vertex_shader: &vs,
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: Some(&fs),
        }).unwrap();
        grr.delete_shaders(&[vs, fs]);

        let vertex_array = grr.create_vertex_array(&[
            // left top
            grr::VertexAttributeDesc {
                location: 0,
                binding: 0,
                format: grr::VertexFormat::Xyz32Float,
                offset: 0,
            },
            // right bottom
            grr::VertexAttributeDesc {
                location: 1,
                binding: 0,
                format: grr::VertexFormat::Xy32Float,
                offset: (3 * std::mem::size_of::<f32>()) as _,
            },
            // left top (tex)
            grr::VertexAttributeDesc {
                location: 2,
                binding: 0,
                format: grr::VertexFormat::Xy32Float,
                offset: (5 * std::mem::size_of::<f32>()) as _,
            },
            // right bottom (tex)
            grr::VertexAttributeDesc {
                location: 3,
                binding: 0,
                format: grr::VertexFormat::Xy32Float,
                offset: (7 * std::mem::size_of::<f32>()) as _,
            },
            // color
            grr::VertexAttributeDesc {
                location: 4,
                binding: 0,
                format: grr::VertexFormat::Xyzw32Float,
                offset: (9 * std::mem::size_of::<f32>()) as _,
            },
        ]).unwrap();

        let sampler = grr.create_sampler(grr::SamplerDesc {
            min_filter: grr::Filter::Linear,
            mag_filter: grr::Filter::Linear,
            mip_map: Some(grr::Filter::Linear),
            address: (
                grr::SamplerAddress::ClampEdge,
                grr::SamplerAddress::ClampEdge,
                grr::SamplerAddress::ClampEdge,
            ),
            lod_bias: 0.0,
            lod: 0.0..1024.0,
            compare: None,
            border_color: [0.0, 0.0, 0.0, 1.0],
        }).unwrap();

        let brush = self.inner.build();

        let glyph_image = {
            let (width, height) = brush.texture_dimensions();
            grr.create_image(grr::ImageType::D2 { width, height, layers: 1, samples: 1 }, grr::Format::R8_UNORM, 1).unwrap()
        };
        let glyph_image_view = grr.create_image_view(
            &glyph_image,
            grr::ImageViewType::D2,
            grr::Format::R8_UNORM,
            grr::SubresourceRange {
                layers: 0..1,
                levels: 0..1,
            },
        ).unwrap();

        GlyphBrush {
            inner: brush,
            glyph_cache: GlyphCache {
                image: glyph_image,
                view: glyph_image_view,
            },
            grr,
            pipeline,
            vertex_array,
            sampler,
            draw: None,
        }
    }
}

struct DrawCommand {
    buffer: grr::Buffer,
    vertices: u32,
}

struct GlyphCache {
    image: grr::Image,
    view: grr::ImageView,
}

pub struct GlyphBrush<'font, 'grr, H = glyph_brush::DefaultSectionHasher> {
    inner: glyph_brush::GlyphBrush<'font, H>,
    grr: &'grr grr::Device,
    pipeline: grr::Pipeline,
    vertex_array: grr::VertexArray,
    sampler: grr::Sampler,
    glyph_cache: GlyphCache,
    draw: Option<DrawCommand>,
}

impl<'font, 'grr> GlyphBrush<'font, 'grr> {
    #[inline]
    pub fn queue_custom_layout<'a, S, G>(&mut self, section: S, custom_layout: &G)
    where
        G: glyph_brush::GlyphPositioner,
        S: Into<Cow<'a, VariedSection<'a>>>,
    {
        self.inner.queue_custom_layout(section, custom_layout)
    }

    #[inline]
    pub fn queue<'a, S>(&mut self, section: S)
    where
        S: Into<Cow<'a, VariedSection<'a>>>,
    {
        self.inner.queue(section)
    }

    #[inline]
    pub fn keep_cached_custom_layout<'a, S, G>(&mut self, section: S, custom_layout: &G)
    where
        G: glyph_brush::GlyphPositioner,
        S: Into<Cow<'a, VariedSection<'a>>>,
    {
        self.inner.keep_cached_custom_layout(section, custom_layout)
    }

    #[inline]
    pub fn keep_cached<'a, S>(&mut self, section: S)
    where
        S: Into<Cow<'a, VariedSection<'a>>>,
    {
        self.inner.keep_cached(section)
    }

    #[inline]
    pub fn draw_queued(
        &mut self,
        dims: (u32, u32),
    ) -> Result<(), String> {
        self.draw_queued_with_transform(IDENTITY_MATRIX4, dims)
    }

    pub fn draw_queued_with_transform(
        &mut self,
        transform: [[f32; 4]; 4],
        dims: (u32, u32),
    ) -> Result<(), String> {
        let mut brush_action;
        loop {
            let grr = self.grr;
            let glyph_cache = &self.glyph_cache;
            brush_action = self.inner.process_queued(
                dims,
                |rect, tex_data| {
                    grr.copy_host_to_image(
                        &glyph_cache.image,
                        grr::SubresourceLevel {
                            level: 0,
                            layers: 0..1,
                        },
                        grr::Offset { x: rect.min.x as _, y: rect.min.y as _, z: 0 },
                        grr::Extent {
                            width: rect.width(),
                            height: rect.height(),
                            depth: 1,
                        },
                        &tex_data,
                        grr::SubresourceLayout {
                            base_format: grr::BaseFormat::R,
                            format_layout: grr::FormatLayout::U8,
                            row_pitch: rect.width(),
                            image_height: rect.height(),
                            alignment: 1,
                        },
                    );
                },
                to_vertex,
            );

            match brush_action {
                Ok(_) => break,
                Err(glyph_brush::BrushError::TextureTooSmall { suggested }) => {
                    unimplemented!()
                }
            }
        }

        match brush_action.unwrap() {
            glyph_brush::BrushAction::Draw(verts) => {
                if let Some(draw) = self.draw.take() {
                    self.grr.delete_buffer(draw.buffer);
                }

                if !verts.is_empty() {
                    self.draw = Some(DrawCommand {
                        buffer:self.grr.create_buffer_from_host(grr::as_u8_slice(verts.as_slice()), grr::MemoryFlags::empty()).unwrap(),
                        vertices: verts.len() as _,
                    });
                }
            }
            glyph_brush::BrushAction::ReDraw => {}
        };

        if let Some(ref cmd) = self.draw {
            let color_blend = grr::ColorBlend {
                attachments: vec![grr::ColorBlendAttachment {
                    blend_enable: true,
                    color: grr::BlendChannel {
                        src_factor: grr::BlendFactor::SrcAlpha,
                        dst_factor: grr::BlendFactor::OneMinusSrcAlpha,
                        blend_op: grr::BlendOp::Add,
                    },
                    alpha: grr::BlendChannel {
                        src_factor: grr::BlendFactor::One,
                        dst_factor: grr::BlendFactor::One,
                        blend_op: grr::BlendOp::Add,
                    },
                }],
            };
            let depth_stencil = grr::DepthStencil {
                depth_test: true,
                depth_write: true,
                depth_compare_op: grr::Compare::LessEqual,
                stencil_test: false,
                stencil_front: grr::StencilFace::KEEP,
                stencil_back: grr::StencilFace::KEEP,
            };

            self.grr.bind_pipeline(&self.pipeline);
            self.grr.bind_vertex_array(&self.vertex_array);
            self.grr.bind_vertex_buffers(&self.vertex_array, 0, &[grr::VertexBufferView {
                buffer: &cmd.buffer,
                offset: 0,
                stride: (std::mem::size_of::<f32>() * 13) as _,
                input_rate: grr::InputRate::Instance { divisor: 1 },
            }]);
            self.grr.bind_color_blend_state(&color_blend);
            self.grr.bind_depth_stencil_state(&depth_stencil);
            self.grr.bind_samplers(0, &[&self.sampler]);
            self.grr.bind_image_views(0, &[&self.glyph_cache.view]);
            self.grr.draw(grr::Primitive::TriangleStrip, 0..4, 0..cmd.vertices as _);
        }

        Ok(())
    }

    #[inline]
    pub fn fonts(&self) -> &[Font<'_>] {
        self.inner.fonts()
    }

    pub fn add_font_bytes<'a: 'font, B: Into<SharedBytes<'a>>>(&mut self, font_data: B) -> glyph_brush::FontId {
        self.inner.add_font_bytes(font_data)
    }

    pub fn add_font<'a: 'font>(&mut self, font_data: Font<'a>) -> glyph_brush::FontId {
        self.inner.add_font(font_data)
    }
}

type Vertex = [f32; 13];

#[inline]
fn to_vertex(
    glyph_brush::GlyphVertex {
        mut tex_coords,
        pixel_coords,
        bounds,
        screen_dimensions: (screen_w, screen_h),
        color,
        z,
    }: glyph_brush::GlyphVertex,
) -> Vertex {
    let gl_bounds = Rect {
        min: point(
            2.0 * (bounds.min.x / screen_w - 0.5),
            2.0 * (0.5 - bounds.min.y / screen_h),
        ),
        max: point(
            2.0 * (bounds.max.x / screen_w - 0.5),
            2.0 * (0.5 - bounds.max.y / screen_h),
        ),
    };

    let mut gl_rect = Rect {
        min: point(
            2.0 * (pixel_coords.min.x as f32 / screen_w - 0.5),
            2.0 * (0.5 - pixel_coords.min.y as f32 / screen_h),
        ),
        max: point(
            2.0 * (pixel_coords.max.x as f32 / screen_w - 0.5),
            2.0 * (0.5 - pixel_coords.max.y as f32 / screen_h),
        ),
    };

    // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if gl_rect.max.x > gl_bounds.max.x {
        let old_width = gl_rect.width();
        gl_rect.max.x = gl_bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * gl_rect.width() / old_width;
    }
    if gl_rect.min.x < gl_bounds.min.x {
        let old_width = gl_rect.width();
        gl_rect.min.x = gl_bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * gl_rect.width() / old_width;
    }
    // note: y access is flipped gl compared with screen,
    // texture is not flipped (ie is a headache)
    if gl_rect.max.y < gl_bounds.max.y {
        let old_height = gl_rect.height();
        gl_rect.max.y = gl_bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
    }
    if gl_rect.min.y > gl_bounds.min.y {
        let old_height = gl_rect.height();
        gl_rect.min.y = gl_bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
    }

    [
        gl_rect.min.x,
        gl_rect.max.y,
        z,
        gl_rect.max.x,
        gl_rect.min.y,
        tex_coords.min.x,
        tex_coords.max.y,
        tex_coords.max.x,
        tex_coords.min.y,
        color[0],
        color[1],
        color[2],
        color[3],
    ]
}
