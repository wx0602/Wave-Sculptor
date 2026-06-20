use std::path::PathBuf;

use clap::Parser;

use crate::audio::analyze::analyze_buffer;
use crate::audio::edit::{
    fade_in_selection, fade_out_selection, mute_selection, normalize_buffer,
    DEFAULT_NORMALIZE_TARGET,
};
use crate::audio::selection::Selection;
use crate::error::{Result, WaveSculptorError};
use crate::wav::{reader, writer};

#[derive(Debug, Parser)]
#[command(
    name = "wave-sculptor",
    version,
    about = "Wave-Sculptor GUI/CLI 音频工具"
)]
pub struct CliArgs {
    pub input: Option<PathBuf>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub stats: bool,
    #[arg(long)]
    pub normalize: bool,
    #[arg(long, num_args = 2, value_names = ["START", "END"])]
    pub mute: Option<Vec<f64>>,
    #[arg(long = "fade-in", num_args = 2, value_names = ["START", "END"])]
    pub fade_in: Option<Vec<f64>>,
    #[arg(long = "fade-out", num_args = 2, value_names = ["START", "END"])]
    pub fade_out: Option<Vec<f64>>,
}

pub fn run(args: CliArgs) -> Result<()> {
    let input = args
        .input
        .ok_or_else(|| WaveSculptorError::Cli("CLI 模式必须提供输入文件".to_string()))?;
    let mut buffer = reader::read_wav_file(&input)?;
    // 只有真正修改音频时才要求输出路径，单纯统计不写文件。
    let mut modified = false;

    if args.stats {
        let analysis = analyze_buffer(&buffer)?;
        println!("文件: {}", input.display());
        println!("时长: {:.3} 秒", buffer.duration_seconds());
        println!("峰值振幅: {:.2}%", analysis.peak * 100.0);
        println!("RMS: {:.4}", analysis.rms);
        println!("削波采样点: {}", analysis.clipping_samples);
        println!("静音片段数: {}", analysis.silent_segments.len());
    }

    if args.normalize {
        normalize_buffer(&mut buffer, DEFAULT_NORMALIZE_TARGET)?;
        modified = true;
    }
    if let Some(range) = args.mute {
        let selection = selection_from_range(&buffer, &range)?;
        mute_selection(&mut buffer, selection)?;
        modified = true;
    }
    if let Some(range) = args.fade_in {
        let selection = selection_from_range(&buffer, &range)?;
        fade_in_selection(&mut buffer, selection)?;
        modified = true;
    }
    if let Some(range) = args.fade_out {
        let selection = selection_from_range(&buffer, &range)?;
        fade_out_selection(&mut buffer, selection)?;
        modified = true;
    }

    if modified {
        let output = args.output.ok_or_else(|| {
            WaveSculptorError::Cli("执行音频修改时必须通过 -o/--output 指定输出文件".to_string())
        })?;
        writer::write_wav_file(&output, &buffer)?;
        println!("已写出: {}", output.display());
    } else if !args.stats {
        return Err(WaveSculptorError::Cli(
            "未指定任何 CLI 操作，可使用 --stats、--normalize、--mute、--fade-in、--fade-out"
                .to_string(),
        ));
    }

    Ok(())
}

fn selection_from_range(
    buffer: &crate::audio::buffer::AudioBuffer,
    range: &[f64],
) -> Result<Selection> {
    // clap 已限制参数个数，这里保留校验以保护直接调用场景。
    if range.len() != 2 {
        return Err(WaveSculptorError::Cli(
            "时间范围参数必须提供开始和结束秒数".to_string(),
        ));
    }

    Selection::from_times(buffer, range[0], range[1])
}
