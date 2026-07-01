//! `inspect <file.band>` — decode a `.band` file and print a hand-written,
//! human-eyeball-only JSON projection of it. This projection is NEVER hashed
//! and never fed back in; it exists purely so a human can sanity-check an
//! import (note counts, tempo, first few events per role) without a full
//! chart viewer. No serde: the JSON is assembled by hand, matching the rest
//! of this crate's "no heavy deps" policy.

use std::process::ExitCode;

use band_core::chart::codec::decode_chart;
use band_core::chart::{ChartCore, DifficultyTier, InstrumentRole};

const ROLE_NAMES: [&str; 4] = ["guitar", "bass", "drums", "vocal"];
const MAX_PREVIEW_NOTES: usize = 5;

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Import always emits exactly one tier per role (Task 3.4 minimal importer), but
/// this is a debug tool over arbitrary decoded `.band` bytes, so fall back to an
/// empty tier rather than panicking if a file somehow has zero tiers for a role.
fn role_tier(core: &ChartCore, role: InstrumentRole) -> DifficultyTier {
    core.tracks[role as usize]
        .tiers
        .first()
        .cloned()
        .unwrap_or_default()
}

fn print_role(core: &ChartCore, role: InstrumentRole, name: &str, is_last: bool) {
    let tier = role_tier(core, role);
    println!("    \"{name}\": {{");
    println!("      \"note_count\": {},", tier.notes.len());
    println!("      \"first_notes\": [");
    let preview: Vec<_> = tier.notes.iter().take(MAX_PREVIEW_NOTES).collect();
    for (i, n) in preview.iter().enumerate() {
        let comma = if i + 1 < preview.len() { "," } else { "" };
        println!(
            "        {{ \"song_pos_us\": {}, \"lanes\": {}, \"space\": {}, \"sustain_len_us\": {}, \"pitch\": {}, \"velocity\": {} }}{comma}",
            n.song_pos_us, n.lanes, n.space as u8, n.sustain_len_us, n.sound.pitch, n.sound.velocity
        );
    }
    println!("      ]");
    let comma = if is_last { "" } else { "," };
    println!("    }}{comma}");
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: inspect <file.band>".to_string());
    }
    let path = &args[0];

    let bytes = std::fs::read(path).map_err(|e| format!("reading '{path}': {e}"))?;
    let chart = decode_chart(&bytes).map_err(|e| format!("decode_chart failed for '{path}': {e:?}"))?;
    let core = &chart.core;

    println!("{{");
    println!("  \"_label\": \"DEBUG PROJECTION \\u2014 NOT HASHED\",");
    println!("  \"source_file\": \"{}\",", json_escape(path));
    println!("  \"title\": \"{}\",", json_escape(&chart.envelope.title));
    println!("  \"duration_us\": {},", core.duration_us);
    match core.tempo_map.first() {
        Some(t) => println!(
            "  \"tempo\": {{ \"micros_per_beat\": {}, \"numerator\": {}, \"denominator\": {} }},",
            t.micros_per_beat, t.numerator, t.denominator
        ),
        None => println!("  \"tempo\": null,"),
    }
    println!("  \"instrument_set_refs\": [{}],",
        core.instrument_set_refs
            .iter()
            .map(|s| format!("\"{}\"", json_escape(&s.id)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("  \"roles\": {{");
    print_role(core, InstrumentRole::Guitar, ROLE_NAMES[0], false);
    print_role(core, InstrumentRole::Bass, ROLE_NAMES[1], false);
    print_role(core, InstrumentRole::Drums, ROLE_NAMES[2], false);
    print_role(core, InstrumentRole::Vocal, ROLE_NAMES[3], true);
    println!("  }}");
    println!("}}");

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
