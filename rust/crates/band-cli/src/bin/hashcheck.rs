//! `hashcheck <file.band>` — decode a `.band` file and print its content-address
//! (BLAKE3 over the hashed `ChartCore` only; the envelope is never hashed).
//! Same input bytes -> same address, always: this is the pipeline's
//! determinism check.

use std::process::ExitCode;

use band_core::chart::codec::decode_chart;
use band_core::chart::hash::content_address;

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: hashcheck <file.band>".to_string());
    }
    let path = &args[0];

    let bytes = std::fs::read(path).map_err(|e| format!("reading '{path}': {e}"))?;
    let chart = decode_chart(&bytes).map_err(|e| format!("decode_chart failed for '{path}': {e:?}"))?;

    println!("{}", content_address(&chart.core));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}
