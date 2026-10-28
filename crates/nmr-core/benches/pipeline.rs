use std::f64::consts::PI;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use nmr_core::{baseline, bruker, decimate, phase, process};

const TD: usize = 65536;
const SW: f64 = 6402.11;

/// A 64k-point int32 FID with a group delay, written to a temporary folder.
fn dataset() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nmr-core-bench-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let acqus = format!(
        "##$TD= {TD}\n##$SW_h= {SW}\n##$SFO1= 400.13\n##$O1= 1880.6\n##$NUC1= <1H>\n\
         ##$AQ_mod= 3\n##$DTYPA= 0\n##$BYTORDA= 0\n##$GRPDLY= 67.984\n##END=\n"
    );
    fs::write(dir.join("acqus"), acqus).unwrap();
    let fid: Vec<u8> = (0..TD / 2)
        .flat_map(|k| {
            let t = k as f64 / SW;
            let amplitude = 1e7 * (-t / 0.8).exp();
            let angle = -2.0 * PI * 1000.0 * t;
            [
                (amplitude * angle.cos()) as i32,
                (amplitude * angle.sin()) as i32,
            ]
        })
        .flat_map(i32::to_le_bytes)
        .collect();
    fs::write(dir.join("fid"), fid).unwrap();
    dir
}

fn benches(c: &mut Criterion) {
    let dir = dataset();
    c.bench_function("read_fid 64k", |b| {
        b.iter(|| bruker::read_fid(black_box(&dir)).unwrap())
    });
    c.bench_function("read_fid + default_1d 64k", |b| {
        b.iter(|| {
            let raw = bruker::read_fid(black_box(&dir)).unwrap();
            process::process(&raw, &process::default_1d(&raw)).unwrap()
        })
    });

    let raw = bruker::read_fid(&dir).unwrap();
    let spectrum = process::process(&raw, &process::default_1d(&raw)).unwrap();
    let real: Vec<f64> = spectrum.data.iter().map(|z| z.re).collect();
    c.bench_function("min_max 131k points to 2000 columns", |b| {
        b.iter(|| decimate::min_max(black_box(&real), 0..real.len(), 2000))
    });
    c.bench_function("airpls baseline 131k points", |b| {
        b.iter(|| baseline::airpls(black_box(&real), 1e7))
    });
    c.bench_function("polynomial baseline order 5, 131k points", |b| {
        b.iter(|| baseline::polynomial(black_box(&real), 5))
    });
    c.bench_function("auto_phase 131k points", |b| {
        b.iter(|| phase::auto_phase(black_box(&spectrum.data), &raw.meta))
    });
    let _ = fs::remove_dir_all(&dir);
}

criterion_group!(pipeline, benches);
criterion_main!(pipeline);
