//! Measure exact lookups in an existing installed corpus; no user queries/history.
use anyhow::{Context, Result, ensure};
use std::{path::PathBuf, time::Instant};
use writy_core::dictionary::Dictionary;
fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("provide an existing dictionary database path")?,
    );
    ensure!(path.is_file(), "dictionary database does not exist");
    let dictionary = Dictionary::open(path)?;
    let sources = dictionary.sources()?;
    let terms = [
        "decision", "language", "write", "good", "computer", "قرار", "كتاب", "لغة",
    ];
    let mut first = Vec::new();
    let mut warm = Vec::new();
    let mut hits = 0;
    for term in terms {
        let start = Instant::now();
        let entries = dictionary.lookup(term)?;
        first.push(start.elapsed().as_micros());
        hits += usize::from(!entries.is_empty());
    }
    for _ in 0..100 {
        for term in terms {
            let start = Instant::now();
            dictionary.lookup(term)?;
            warm.push(start.elapsed().as_micros());
        }
    }
    warm.sort_unstable();
    println!(
        "{}",
        serde_json::json!({"sources":sources.len(),"entries":sources.iter().map(|(_,n)|n).sum::<usize>(),"sample_terms":terms.len(),"sample_hits":hits,"first_lookup_us":first,"warm_samples":warm.len(),"warm_p50_us":warm[warm.len()/2],"warm_p95_us":warm[warm.len()*95/100]})
    );
    Ok(())
}
