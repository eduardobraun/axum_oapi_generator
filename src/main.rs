use std::path::PathBuf;

use clap::Parser;
use openapiv3::OpenAPI;

mod generator;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to OpenApi spec file
    spec: PathBuf,
    /// Output directory
    out_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let content = std::fs::read_to_string(cli.spec)?;
    let schema = serde_yaml::from_str::<OpenAPI>(&content)?;
    let mut state = generator::OapiState::new(schema);

    let files = generator::generate(&mut state)?;
    for (_file, content) in files {
        println!("{}", content);
    }

    Ok(())
}
