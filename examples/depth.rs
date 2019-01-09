use std::{error::Error};
use glutin::GlContext;
use glyph_brush::rusttype::Scale;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut events_loop = glutin::EventsLoop::new();
    let title = "grr-glyph example - depth handling";
    let window = glutin::GlWindow::new(
        glutin::WindowBuilder::new()
            .with_dimensions((700, 320).into())
            .with_title(title),
        glutin::ContextBuilder::new()
            .with_depth_buffer(24)
            .with_srgb(true),
        &events_loop,
    )?;
    unsafe { window.make_current()? };

    let grr = grr::Device::new(
        |symbol| window.get_proc_address(symbol) as *const _,
        grr::Debug::Enable {
            callback: |_, _, _, _, msg| {
                println!("{:?}", msg);
            },
            flags: grr::DebugReport::FULL,
        },
    );

    let fonts: Vec<&[u8]> = vec![
        include_bytes!("fonts/FiraSans-Regular.ttf"),
    ];
    let italic_font = glyph_brush::FontId(0);

    let mut glyph_brush = grr_glyph::GlyphBrushBuilder::using_fonts_bytes(fonts)
        .initial_cache_size((512, 512))
        .build(&grr);

    let mut dimensions = window
        .get_inner_size()
        .ok_or("get_inner_size = None")?
        .to_physical(window.get_hidpi_factor());

    let mut running = true;
    while running {
        events_loop.poll_events(|event| {
            use glutin::*;
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    }
                    | WindowEvent::CloseRequested => running = false,
                    WindowEvent::Resized(size) => {
                        let dpi = window.get_hidpi_factor();
                        window.resize(size.to_physical(dpi));
                        if let Some(ls) = window.get_inner_size() {
                            dimensions = ls.to_physical(dpi);
                            grr.set_viewport(
                                0,
                                &[grr::Viewport {
                                    x: 0.0,
                                    y: 0.0,
                                    w: dimensions.width as _,
                                    h: dimensions.height as _,
                                    n: 0.0,
                                    f: 1.0,
                                }],
                            );
                            grr.set_scissor(
                                0,
                                &[grr::Region {
                                    x: 0,
                                    y: 0,
                                    w: dimensions.width as _,
                                    h: dimensions.height as _,
                                }],
                            );
                        }
                    }
                    _ => {}
                }
            }
        });

        let width = dimensions.width as f32;
        let height = dimensions.height as _;

        grr.clear_attachment(
            grr::Framebuffer::DEFAULT,
            grr::ClearAttachment::ColorFloat(0, [0.02, 0.02, 0.02, 1.0]),
        );
        grr.clear_attachment(
            grr::Framebuffer::DEFAULT,
            grr::ClearAttachment::Depth(1.0),
        );

        // first section is queued, and therefore drawn, first with lower z
        glyph_brush.queue(glyph_brush::Section {
            screen_position: (width / 2.0, 100.0),
            bounds: (width, height - 100.0),
            text: "grr!",
            scale: Scale::uniform(95.0),
            color: [0.8, 0.8, 0.8, 1.0],
            font_id: italic_font,
            layout: glyph_brush::Layout::default().h_align(glyph_brush::HorizontalAlign::Center),
            z: 0.2,
        });

        // 2nd section is drawn last but with higher z,
        // draws are subject to depth testing
        glyph_brush.queue(glyph_brush::Section {
            bounds: (width, height),
            text: &include_str!("text/lipsum.txt").replace("\n\n", "").repeat(10),
            scale: Scale::uniform(30.0),
            color: [0.05, 0.05, 0.1, 1.0],
            z: 1.0,
            ..glyph_brush::Section::default()
        });

        glyph_brush.draw_queued((width as _, height as _))?;

        window.swap_buffers()?;
    }
    Ok(())
}
