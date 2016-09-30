//! QuickFLI.

extern crate flic;
extern crate sdl2;

use std::env;
use std::path::PathBuf;
use flic::{FlicFile,RasterMut};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

const DEFAULT_SCREEN_WIDTH: u32 = 640;
const DEFAULT_SCREEN_HEIGHT: u32 = 400;
const MIN_SCREEN_WIDTH: u32 = 320;
const MIN_SCREEN_HEIGHT: u32 = 200;
const FLIC_WIDTH: usize = 320;
const FLIC_HEIGHT: usize = 200;

fn main() {
    let mut filenames: Vec<String> = env::args().skip(1).collect();
    let mut flic: Option<FlicFile> = None;
    let mut next_file: usize = 0;

    usage();
    if filenames.len() <= 0 {
        return;
    }

    // Initialise SDL window.
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();

    let mut window
        = video.window("QuickFLI", DEFAULT_SCREEN_WIDTH, DEFAULT_SCREEN_HEIGHT)
        .resizable()
        .position_centered()
        .opengl()
        .build().unwrap();

    let _ = window.set_minimum_size(MIN_SCREEN_WIDTH, MIN_SCREEN_HEIGHT);
    let mut renderer = window.renderer().build().unwrap();
    let mut timer = sdl.timer().unwrap();
    let mut event_pump = sdl.event_pump().unwrap();

    let mut texture = renderer.create_texture_streaming(
            PixelFormatEnum::RGB24, FLIC_WIDTH as u32, FLIC_HEIGHT as u32).unwrap();
    let mut buf = vec![0; FLIC_WIDTH * FLIC_HEIGHT];
    let mut pal = vec![0; 3 * 256];

    let mut last_tstart: u32 = 0;
    'mainloop: loop {
        let msec = match flic {
            Some(ref f) => 1000 * f.speed_jiffies() as u32 / 70,
            None => 100,
        };

        let tnow = timer.ticks();
        let tstart = if msec > 0 { tnow - tnow % msec } else { tnow };
        let tend = tstart + msec;

        let redraw = tstart > last_tstart;
        if redraw {
            if let Some(ref mut flic) = flic {
                match flic.read_next_frame(
                        &mut RasterMut::new(FLIC_WIDTH, FLIC_HEIGHT, &mut buf, &mut pal)) {
                    Ok(_) => {
                        render_to_texture(&mut texture, FLIC_WIDTH, FLIC_HEIGHT, &buf, &pal);
                        present_to_screen(&mut renderer, &texture);
                    },
                    Err(e) => {
                        println!("Error occurred - {}", e);
                    },
                }
            } else {
                renderer.clear();
                renderer.present();
            }

            last_tstart = tstart;
        }

        if !redraw || msec == 0 {
            if let Some(e) = event_pump.wait_event_timeout(tend - tnow) {
                match e {
                    Event::Quit {..}
                    | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                        break 'mainloop;
                    },

                    Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                        if filenames.len() > 1 {
                            flic = None;
                        }
                    },

                    _ => (),
                }
            }

            // Try loading a new flic.
            if flic.is_none() && filenames.len() > 0 {
                if next_file >= filenames.len() {
                    next_file = 0;
                }

                let path = PathBuf::from(&filenames[next_file]);
                match FlicFile::open(path.as_path()) {
                    Ok(f) => {
                        println!("Loaded {}", &filenames[next_file]);
                        flic = Some(f);
                        last_tstart = 0;
                        next_file = next_file + 1;
                    },
                    Err(e) => {
                        println!("Error loading {} -- {}", &filenames[next_file], e);
                        filenames.remove(next_file);
                    },
                }
            }
        }
    }
}

fn usage() {
    println!("QuickFLI - a simple player for 256 color VGA animations");
    println!("");
    println!("Give QuickFLI a list of FLICs to play.");
    println!("<ESC> to abort playback.");
    println!("<space> to go to next FLIC.");
}

fn render_to_texture(
        texture: &mut sdl2::render::Texture,
        w: usize, h: usize, buf: &[u8], pal: &[u8]) {
    texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
        for y in 0..h {
            for x in 0..w {
                let offset = pitch * y + 3 * x;
                let c = buf[w * y + x] as usize;
                let r = pal[3 * c + 0];
                let g = pal[3 * c + 1];
                let b = pal[3 * c + 2];

                buffer[offset + 0] = (r << 2) | (r >> 4);
                buffer[offset + 1] = (g << 2) | (g >> 4);
                buffer[offset + 2] = (b << 2) | (b >> 4);
            }
        }
    }).unwrap();
}

fn present_to_screen(
        renderer: &mut sdl2::render::Renderer,
        texture: &sdl2::render::Texture) {
    renderer.clear();
    renderer.copy(&texture, None, None);
    renderer.present();
}
