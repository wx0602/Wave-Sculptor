use clap::Parser;
use wave_sculptor::cli::CliArgs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();
    if args.input.is_some() {
        wave_sculptor::cli::run(args)?;
    } else {
        wave_sculptor::gui::run()?;
    }

    Ok(())
}
