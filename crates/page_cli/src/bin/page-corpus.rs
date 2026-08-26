use clap::Parser;

#[path = "../corpus.rs"]
mod corpus;

#[derive(Debug, Parser)]
#[command(
    name = "page-corpus",
    version,
    about = "Run the internal veraPDF corpus validation gate"
)]
struct Cli {
    #[command(flatten)]
    args: corpus::CorpusArgs,
}

fn main() {
    std::process::exit(corpus::run(&Cli::parse().args));
}
