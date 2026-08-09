use std::f64::consts::PI;

use sdl3::{self, keyboard::Keycode, pixels::Color, sys::render::SDL_RendererLogicalPresentation};

fn main() {
    let sdl = sdl3::init().unwrap();

    let video_sys = sdl.video().unwrap();
    let window = video_sys
        .window("lithia-powder", 640, 480)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas();
    canvas
        .set_logical_size(640, 480, SDL_RendererLogicalPresentation::LETTERBOX)
        .unwrap();

    let event_sys = sdl.event().unwrap();
    let mut event_pump = sdl.event_pump().unwrap();

    'main_loop: loop {
        'event_loop: loop {
            let Some(event) = event_pump.poll_event() else {
                break 'event_loop;
            };
            match event {
                sdl3::event::Event::Quit { .. } => {
                    break 'main_loop;
                }
                sdl3::event::Event::KeyDown {
                    timestamp, keycode, ..
                } => {
                    if keycode == Some(Keycode::Escape) {
                        event_sys
                            .push_event(sdl3::event::Event::Quit { timestamp })
                            .unwrap();
                    }
                }
                _ => {}
            }
        }

        let color = {
            let ticks = sdl3::timer::ticks() as f64 / 1000.0;
            let red = 0.5 * (1.0 + f64::sin(ticks));
            let green = 0.5 * (1.0 + f64::sin(ticks + PI * 2. / 3.));
            let blue = 0.5 * (1.0 + f64::sin(ticks + PI * 4. / 3.));
            Color::RGB(
                (red * u8::MAX as f64) as u8,
                (green * u8::MAX as f64) as u8,
                (blue * u8::MAX as f64) as u8,
            )
        };

        canvas.set_draw_color(color);
        canvas.clear();
        canvas.present();
    }

    println!("bye bye!");

    ()
}
