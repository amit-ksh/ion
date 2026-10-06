//! Internal worker transport; stdin/stdout are bounded versioned JSON, stderr is diagnostic.
use std::io::{Read, Write};
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("ion-harness-worker accepts one versioned JSON request on stdin, no arguments");
        std::process::exit(2);
    }
    let result = (|| -> anyhow::Result<()> {
        let mut input = Vec::new();
        std::io::stdin()
            .take(128 * 1024 + 1)
            .read_to_end(&mut input)?;
        let value = waku_core::harness_worker::execute(&input)?;
        std::io::stdout().write_all(&serde_json::to_vec(&value)?)?;
        Ok(())
    })();
    if result.is_err() {
        eprintln!(
            "Ion harness worker failed. Verify native harness authentication, exact model metadata, worker version and allowance. No error payload or credentials are returned."
        );
        std::process::exit(1);
    }
}
