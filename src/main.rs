use clap::Parser;
use sendit::Cli;

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    std::process::exit(sendit::run(cli));
}
