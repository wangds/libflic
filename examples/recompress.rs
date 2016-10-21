//! Recompress FLI files.

extern crate flic;

use std::env;
use std::path::{Path,PathBuf};
use flic::{FlicFile,FlicFileWriter,FlicResult,Raster,RasterMut};

fn main() {
    if env::args().count() <= 1 {
        usage();
        return;
    }

    for filename in env::args().skip(1) {
        let filepath = Path::new(&filename);
        let outname = if let Some(filename) = filepath.file_name() {
            let mut outname = PathBuf::new();
            outname.set_file_name(filename);
            outname.set_extension("tmp");
            outname
        } else {
            continue;
        };

        let mut fin = match FlicFile::open(&filepath) {
            Ok(flic) => flic,
            Err(e) => {
                println!("Error reading {} - {}",
                        filename, e);
                continue;
            },
        };

        let mut fout = match FlicFileWriter::create(
                outname.as_path(),
                fin.width(), fin.height(), fin.speed_msec()) {
            Ok(flic) => flic,
            Err(e) => {
                println!("Error writing {} - {}",
                        outname.to_string_lossy(), e);
                continue;
            },
        };

        println!("{} -> {}", filename, outname.to_string_lossy());

        match recompress(&mut fin, &mut fout) {
            Ok(_) => {
                let _ = fout.close();
            },
            Err(e) => {
                println!("Error occurred - {}", e);
            },
        }
    }
}

fn usage() {
    println!("Usage: recompress <FLIC files>");
}

fn recompress(fin: &mut FlicFile, fout: &mut FlicFileWriter)
        -> FlicResult<()> {
    let w = fin.width() as usize;
    let h = fin.height() as usize;

    fout.set_creator(fin.creator(), fin.creation_time());
    fout.set_aspect_ratio(fin.aspect_x(), fin.aspect_y());

    let mut buf_prev = vec![0; w * h];
    let mut buf_next = vec![0; w * h];
    let mut pal_prev = [0; 3 * 256];
    let mut pal_next = [0; 3 * 256];
    let mut first = true;

    loop {
        let res = try!(fin.read_next_frame(
                &mut RasterMut::new(w, h, &mut buf_next, &mut pal_next)));

        if first {
            try!(fout.write_next_frame(
                    None,
                    &Raster::new(w, h, &buf_next, &pal_next)));
        } else {
            try!(fout.write_next_frame(
                    Some(&Raster::new(w, h, &buf_prev, &pal_prev)),
                    &Raster::new(w, h, &buf_next, &pal_next)));
        }

        if res.looped {
            break;
        } else {
            buf_prev.copy_from_slice(&buf_next);
            pal_prev.copy_from_slice(&pal_next);
            first = false;
        }
    }

    Ok(())
}
