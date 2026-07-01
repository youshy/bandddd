//! `import <in.mid> <out.band> [--guitar N --bass N --drums N --vocal N]`
//!
//! Reads a Standard MIDI File, reduces the chosen role tracks to a one-tier
//! chart (`band_core::midi_import::import_chart`), sets the (unhashed)
//! envelope title from the input filename stem, and writes the canonical
//! `.band` encoding to disk. Native-only harness — not wasm-clean, no serde,
//! no arg-parser crate: flags are hand-parsed.

use std::path::Path;
use std::process::ExitCode;

use band_core::chart::codec::encode_chart;
use band_core::midi_import::{import_chart, RoleMapping};

fn usage() -> String {
    "usage: import <in.mid> <out.band> [--guitar N] [--bass N] [--drums N] [--vocal N]".to_string()
}

/// Hand-parse `--role N` flags (any order, all optional) into a `RoleMapping`.
fn parse_mapping(args: &[String]) -> Result<RoleMapping, String> {
    let mut mapping = RoleMapping::default();
    let mut i = 0;
    while i < args.len() {
        let flag = &args[i];
        let slot = match flag.as_str() {
            "--guitar" => &mut mapping.guitar,
            "--bass" => &mut mapping.bass,
            "--drums" => &mut mapping.drums,
            "--vocal" => &mut mapping.vocal,
            other => return Err(format!("unknown flag: {other}")),
        };
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a track-index argument"))?;
        let idx: usize = value
            .parse()
            .map_err(|_| format!("{flag} value must be a non-negative integer, got '{value}'"))?;
        *slot = Some(idx);
        i += 2;
    }
    Ok(mapping)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err(usage());
    }
    let in_path = &args[0];
    let out_path = &args[1];
    let mapping = parse_mapping(&args[2..])?;

    let smf_bytes = std::fs::read(in_path).map_err(|e| format!("reading '{in_path}': {e}"))?;

    let mut chart = import_chart(&smf_bytes, mapping)
        .map_err(|e| format!("import_chart failed for '{in_path}': {e:?}"))?;

    let title = Path::new(in_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();
    chart.envelope.title = title;

    let bytes = encode_chart(&chart);
    std::fs::write(out_path, &bytes).map_err(|e| format!("writing '{out_path}': {e}"))?;

    println!(
        "imported '{in_path}' -> '{out_path}' ({} bytes, title='{}')",
        bytes.len(),
        chart.envelope.title
    );
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
