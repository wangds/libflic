//! FLIC browser.

extern crate flic;
extern crate sdl2;

use std::env;
use std::path::Path;
use flic::{FlicFile,RasterMut};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

const SCREEN_W: u32 = 640;
const SCREEN_H: u32 = 400;
const MIN_SCREEN_W: u32 = 320;
const MIN_SCREEN_H: u32 = 200;
const MAX_PSTAMP_W: u16 = 98;
const MAX_PSTAMP_H: u16 = 61;

fn main() {
    let dirname = env::args().nth(1);
    if dirname.is_none() {
        usage();
        return;
    }

    let dir = Path::new(dirname.as_ref().unwrap());
    if !dir.is_dir() {
        usage();
        return;
    }

    // Initialise SDL window.
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();

    let mut window
        = video.window("FLIC Browser", SCREEN_W, SCREEN_H)
        .resizable()
        .position_centered()
        .opengl()
        .build().unwrap();

    let _ = window.set_minimum_size(MIN_SCREEN_W, MIN_SCREEN_H);
    let mut renderer = window.renderer().build().unwrap();
    let mut event_pump = sdl.event_pump().unwrap();
    let mut texture = renderer.create_texture_streaming(
            PixelFormatEnum::RGB24, SCREEN_W, SCREEN_H).unwrap();

    if let Ok(entries) = dir.read_dir() {
        let mut filenames = Vec::new();
        let mut count = 0;

        // Find FLIC files by extension.
        for entry in entries {
            if entry.is_err() {
                continue;
            }

            let path = entry.unwrap().path();
            if !path.is_file() || path.extension().is_none() {
                continue;
            }

            // Surely something better is possible...
            let ext = path.extension().unwrap()
                    .to_os_string().into_string().unwrap()
                    .to_lowercase();
            if ext != "flc" && ext != "fli" {
                continue;
            }

            filenames.push(path);
        }

        filenames.sort();

        let mut buf = [0; (SCREEN_W * SCREEN_H) as usize];
        let mut pal = [0; 3 * 256];

        // Render postage stamps.
        for filename in filenames {
            let mut flic = match FlicFile::open(&filename) {
                Ok(f) => f,
                Err(e) => {
                    println!("Error loading {} -- {}",
                            filename.to_string_lossy(), e);
                    continue;
                },
            };

            let (pstamp_w, pstamp_h) = flic::pstamp::get_pstamp_size(
                    MAX_PSTAMP_W, MAX_PSTAMP_H, flic.width(), flic.height());

            let gridx = count % 6;
            let gridy = count / 6;
            let x = 27 + 102 * gridx + (MAX_PSTAMP_W - pstamp_w) / 2;
            let y =  1 +  80 * gridy + (MAX_PSTAMP_H - pstamp_h) / 2;

            {
                let mut raster = RasterMut::with_offset(
                        x as usize, y as usize,
                        pstamp_w as usize, pstamp_h as usize, SCREEN_W as usize,
                        &mut buf, &mut pal);
                if let Err(e) = flic.read_postage_stamp(&mut raster) {
                    println!("Error reading postage stamp -- {}", e);
                    continue;
                }
            }

            draw_rect(&mut buf, x - 1, y - 1, pstamp_w + 2, pstamp_h + 2);

            count = count + 1;
            if count >= 6 * 5 {
                break;
            }
        }

        pal[3 * 255 + 0] = 0x98;
        pal[3 * 255 + 1] = 0x98;
        pal[3 * 255 + 2] = 0x98;
        render_to_texture(&mut texture,
                SCREEN_W as usize, SCREEN_H as usize, &buf, &pal);
    }

    present_to_screen(&mut renderer, &texture);

    'mainloop: loop {
        if let Some(e) = event_pump.wait_event_timeout(100) {
            match e {
                Event::Quit {..}
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'mainloop;
                },

                _ => (),
            }
        } else {
            present_to_screen(&mut renderer, &texture);
        }
    }
}

fn usage() {
    println!("Usage: browse <directory containing FLIC files>");
}

fn draw_rect(
        buf: &mut [u8], x: u16, y: u16, w: u16, h: u16) {
    let x = x as u32;
    let y = y as u32;
    let w = w as u32;
    let h = h as u32;
    let stride = SCREEN_W;
    let c = 0xFF;

    for i in x..(x + w) {
        buf[(stride * y + i) as usize] = c;
    }

    for i in y..(y + h) {
        buf[(stride * i + x) as usize] = c;
    }

    for i in y..(y + h) {
        buf[(stride * i + (x + w - 1)) as usize] = c;
    }

    for i in x..(x + w) {
        buf[(stride * (y + h - 1) + i) as usize] = c;
    }
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
    let _ = renderer.copy(&texture, None, None);
    renderer.present();
}
