#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::io::{Cursor, Write};

use rand::Rng;
use rocket::http::ContentType;
use rocket::Response;
use rocket::response::Redirect;

fn genPallet() -> Vec<u8> {
    let mut rng = rand::thread_rng();

    let rstep = rng.gen_range(0, 20);
    let gstep = rng.gen_range(0, 20);
    let bstep = rng.gen_range(0, 20);

    let mut v = Vec::new();
    let mut r: u16 = 0;
    let mut g: u16 = 0;
    let mut b: u16 = 0;
    for _i in 0..256 {
        v.write(&[r as u8, g as u8, b as u8]);
        r = (r + rstep) % 255;
        g = (g + gstep) % 255;
        b = (b + bstep) % 255;
    }
    v
}


fn mbrot(x: f64, y: f64) -> u8 {
    let mut i = 0;
    let mut a = 0.0;
    let mut b = 0.0;

    while (i < 254) && ((a * a) + (b * b) < 4.0) {
        i += 1;
        let a0 = a;
        let b0 = b;
        a = (a0 * a0) - (b0 * b0) + x;
        b = (2.0 * a0 * b0) + y;
    }

    i
}


#[get("/mandelbrot/render?<crmin>&<crmax>&<cimin>&<cimax>")]
fn mandelbrot<'a>(crmin: f64, crmax: f64, cimin: f64, cimax: f64) -> Response<'a> {
    let pal = genPallet();


    let mut raw_bytes: Vec<u8> = Vec::new();

    let xsize = 1600;
    let ysize = 1200;
    let rstep = (crmax - crmin) / xsize as f64;
    let istep = (cimax - cimin) / ysize as f64;

    {
        let mut encoder = png::Encoder::new(&mut raw_bytes, xsize, ysize);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().unwrap();
        writer.write_chunk(png::chunk::PLTE, pal.as_ref());

        let mut stream_writer = writer.stream_writer();

        let mut ci = cimin;
        for _yp in 0..ysize {
            let mut cr = crmin;
            let mut row = Vec::new();
            for _xp in 0..xsize {
                row.push(mbrot(cr, ci));
                cr += rstep;
            }
            stream_writer.write(row.as_slice()).unwrap();
            ci += istep;
        }
        stream_writer.finish();
    }

    Response::build()
        .header(ContentType::PNG)
        .streamed_body(Cursor::new(raw_bytes))
        .finalize()
}


#[get("/")]
fn index() -> Redirect {
    rocket::response::Redirect::to(uri!(mandelbrot: -2.0, 1.0, -1.0, 1.0))
}

fn main() {
    rocket::ignite().mount("/", routes![index, mandelbrot]).launch();
}
