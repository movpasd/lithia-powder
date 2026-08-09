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

    let mut event_pump = sdl.event_pump().unwrap();

    'main_loop: loop {
        for event in event_pump.poll_iter() {
            match event {
                sdl3::event::Event::Quit { .. }
                | sdl3::event::Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'main_loop;
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
