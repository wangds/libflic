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

    let mut flic_w = 0;
    let mut flic_h = 0;
    let mut texture = None;
    let mut buf = vec![0; flic_w * flic_h];
    let mut pal = vec![0; 3 * 256];

    let mut last_tstart: u32 = 0;
    'mainloop: loop {
        let msec = match flic {
            Some(ref f) => f.speed_msec(),
            None => 100,
        };

        let tnow = timer.ticks();
        let tstart = if msec > 0 { tnow - tnow % msec } else { tnow };
        let tend = tstart + msec;

        let redraw = tstart > last_tstart;
        if redraw {
            if let (Some(ref mut flic), Some(ref mut texture)) = (flic.as_mut(), texture.as_mut()) {
                match flic.read_next_frame(
                        &mut RasterMut::new(flic_w, flic_h, &mut buf, &mut pal)) {
                    Ok(_) => {
                        render_to_texture(texture, flic_w, flic_h, &buf, &pal);
                        present_to_screen(&mut renderer, texture);
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

                        if f.width() as usize != flic_w || f.height() as usize != flic_h {
                            flic_w = f.width() as usize;
                            flic_h = f.height() as usize;

                            texture = renderer.create_texture_streaming(
                                        PixelFormatEnum::RGB24,
                                        flic_w as u32, flic_h as u32).ok();
                            buf = vec![0; flic_w * flic_h];
                        }

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

                buffer[offset + 0] = pal[3 * c + 0];
                buffer[offset + 1] = pal[3 * c + 1];
                buffer[offset + 2] = pal[3 * c + 2];
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
