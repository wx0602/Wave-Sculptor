use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, WaveSculptorError>;

#[derive(Debug, Error)]
pub enum WaveSculptorError {
    #[error("输入输出错误：{0}")]
    Io(#[from] io::Error),
    #[error("不支持的 WAV 格式：{0}")]
    UnsupportedFormat(String),
    #[error("无效的 WAV 文件：{0}")]
    InvalidWav(String),
    #[error("缺少必需的数据块：{0}")]
    MissingChunk(&'static str),
    #[error("无效的选区范围")]
    InvalidSelection,
    #[error("当前未加载音频文件")]
    NoAudioLoaded,
    #[error("播放错误：{0}")]
    Playback(String),
    #[error("无效参数：{0}")]
    InvalidParameter(String),
    #[error("命令行参数错误：{0}")]
    Cli(String),
}
