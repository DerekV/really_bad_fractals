#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use std::io::{Cursor, Write};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::mpsc;

use rand::Rng;
use rocket::http::ContentType;
use rocket::Response;
use rocket::response::Redirect;
use threadpool::ThreadPool;

use log::{debug};

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


fn mandlebrot_point(x: f64, y: f64, max_iter: u32) -> u32 {
    let mut i = 0;
    let mut a = 0.0;
    let mut b = 0.0;

    while (i < max_iter) && ((a * a) + (b * b) < 4.0) {
        i += 1;
        let a0 = a;
        let b0 = b;
        a = (a0 * a0) - (b0 * b0) + x;
        b = (2.0 * a0 * b0) + y;
    }

    i
}

#[derive(Copy, Clone, Debug)]
struct MbrotRequest {
    max_iter: u32,
    xsize: u32,
    ysize: u32,
    crmax: f64,
    crmin: f64,
    cimax: f64,
    cimin: f64,
}

struct MbrotResponse {
    request: MbrotRequest,
    data: Vec<Vec<u32>>,
}


fn render_mandelbrot_simple(request: MbrotRequest) -> MbrotResponse {
    let rstep = (request.crmax - request.crmin) / request.xsize as f64;
    let istep = (request.cimax - request.cimin) / request.ysize as f64;
    let mut data = Vec::new();
    let mut ci = request.cimin;
    for _yp in 0..request.ysize {
        let mut row = Vec::new();
        let mut cr = request.crmin;
        for _xp in 0..request.xsize {
            row.push(mandlebrot_point(cr, ci, request.max_iter));
            cr += rstep;
        }
        data.push(row);
        ci += istep;
    }

    MbrotResponse {
        request,
        data,
    }
}

#[get("/mandelbrot/render?<crmin>&<crmax>&<cimin>&<cimax>&<max_iter>&<xsize>&<ysize>")]
fn mandelbrot<'a>(crmin: f64, crmax: f64, cimin: f64, cimax: f64, max_iter: u32, xsize: Option<u32>, ysize: Option<u32>) -> Response<'a> {
    let pal = genPallet();


    let xs = xsize.unwrap_or(800);
    let ys = ysize.unwrap_or(600);
    let rstep = (crmax - crmin) / xs as f64;
    let istep = (cimax - cimin) / ys as f64;

    let (tx, rx): (Sender<MbrotResponse>, Receiver<MbrotResponse>) = mpsc::channel();

    let pool = ThreadPool::with_name("mbrot_worker".into(), 8);
    let mut ci = cimin;
    for row in 0..ys {
        //let pooli = pool.clone();  // cloning pool is correct way to share with threads

        let tx = mpsc::Sender::clone(&tx); // cloning channel is correct way to share with threads
        let req = MbrotRequest {
            xsize: xs,
            ysize: 1,
            crmin,
            crmax,
            cimin: ci,
            cimax: ci,
            max_iter,
        };
        debug!("pooling request row {}", row);
        pool.execute(move || {
            debug!("executing request row {}: got request {:?}", row, req);
            tx.send(render_mandelbrot_simple(req)).expect("Channel should be open");
            debug!("finished row {}", row)
        });
        ci += istep;
    };
    drop(tx);

    debug!("waiting to join");
    pool.join();

    debug!("Join complete");

    let mut raw_bytes: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut raw_bytes, xs, ys);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().unwrap();
        writer.write_chunk(png::chunk::PLTE, pal.as_ref());

        let mut stream_writer = writer.stream_writer();

        let mut responses: Vec<MbrotResponse> = rx.into_iter().collect();
        responses.sort_by(|a, b| a.request.cimin.partial_cmp(&b.request.cimin).expect("imaginary component should not be NaN"));
        for r in responses {
            debug!("received response for {:?}", r.request);
            // this is going to be all jambled up, but i just want to see the results at the end
            for supposedly_row in r.data {
                let row: Vec<u8> = supposedly_row.into_iter().map(|n| (n % 256) as u8).collect();
                stream_writer.write(row.as_slice()).unwrap();
            }
        }
        stream_writer.finish();
    }


//        {
//
//            let mut encoder = png::Encoder::new(&mut raw_bytes, xsize, ysize);
//            encoder.set_color(png::ColorType::Indexed);
//            encoder.set_depth(png::BitDepth::Eight);
//
//            let mut writer = encoder.write_header().unwrap();
//            writer.write_chunk(png::chunk::PLTE, pal.as_ref());
//
//            let mut stream_writer = writer.stream_writer();
//
//            let mut ci = cimin;
//            for _yp in 0..ysize {
//                let mut cr = crmin;
//                let mut row = Vec::new();
//                for _xp in 0..xsize {
//                    row.push(mbrot(cr, ci));
//                    cr += rstep;
//                }
//                stream_writer.write(row.as_slice()).unwrap();
//                ci += istep;
//            }
//            stream_writer.finish();
//        }
//        raw_bytes
//    }
// todo stuff

    Response::build()
        .header(ContentType::PNG)
        .streamed_body(Cursor::new(raw_bytes))
        .finalize()
}


#[get("/")]
fn index() -> Redirect {
    rocket::response::Redirect::to(uri!(mandelbrot: - 2.0, 1.0, - 1.0, 1.0, 300, 800, 600))
}

fn main() {
    rocket::ignite().mount("/", routes![index, mandelbrot]).launch();
}
