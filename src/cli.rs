use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "conspectus", version, about = "AI work graph status tool")]
pub struct Cli {}

impl Cli {
    pub fn run(self) {}
}
