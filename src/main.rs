use clap::Parser;
use wave_sculptor::cli::CliArgs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();
    // 有输入文件时走 CLI 批处理，否则启动交互式 GUI。
    if args.input.is_some() {
        wave_sculptor::cli::run(args)?;
    } else {
        wave_sculptor::gui::run()?;
    }

    Ok(())
}
