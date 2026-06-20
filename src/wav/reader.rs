use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::audio::buffer::AudioBuffer;
use crate::error::{Result, WaveSculptorError};

#[derive(Clone, Debug, PartialEq)]
pub struct WavHeader {
    pub audio_format: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub data_size: u32,
}

pub fn read_wav_file(path: &Path) -> Result<AudioBuffer> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    parse_wav(&mut reader)
}

#[cfg(test)]
pub fn parse_header<R: Read + Seek>(reader: &mut R) -> Result<WavHeader> {
    let (header, _) = locate_chunks(reader)?;
    Ok(header)
}

pub fn parse_wav<R: Read + Seek>(reader: &mut R) -> Result<AudioBuffer> {
    let (header, data_offset) = locate_chunks(reader)?;

    // 处理链路只支持未压缩 16 位 PCM，避免后续采样解释出错。
    if header.audio_format != 1 {
        return Err(WaveSculptorError::UnsupportedFormat(format!(
            "仅支持 PCM WAV，当前编码格式编号为 {}",
            header.audio_format
        )));
    }
    if header.bits_per_sample != 16 {
        return Err(WaveSculptorError::UnsupportedFormat(format!(
            "仅支持 16 位 PCM WAV，当前每个采样为 {} 位",
            header.bits_per_sample
        )));
    }
    if !(1..=2).contains(&header.channels) {
        return Err(WaveSculptorError::UnsupportedFormat(format!(
            "仅支持单声道或立体声 WAV，当前为 {} 个声道",
            header.channels
        )));
    }
    if header.data_size % 2 != 0 {
        return Err(WaveSculptorError::InvalidWav(
            "对于 16 位 PCM，音频数据块大小必须为偶数".to_string(),
        ));
    }

    reader.seek(SeekFrom::Start(data_offset))?;
    let mut data = vec![0_u8; header.data_size as usize];
    reader.read_exact(&mut data)?;

    // 16 位 PCM 采样在 WAV 中按小端序连续存放。
    let samples = data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    Ok(AudioBuffer::new(
        samples,
        header.sample_rate,
        header.channels,
        header.bits_per_sample,
    ))
}

fn locate_chunks<R: Read + Seek>(reader: &mut R) -> Result<(WavHeader, u64)> {
    reader.seek(SeekFrom::Start(0))?;

    // RIFF/WAVE 文件由若干 chunk 组成，fmt 和 data 的顺序不固定。
    let mut riff = [0_u8; 4];
    reader.read_exact(&mut riff)?;
    if &riff != b"RIFF" {
        return Err(WaveSculptorError::InvalidWav(
            "缺少 RIFF 文件标识".to_string(),
        ));
    }

    let _file_size = read_u32_le(reader)?;

    let mut wave = [0_u8; 4];
    reader.read_exact(&mut wave)?;
    if &wave != b"WAVE" {
        return Err(WaveSculptorError::InvalidWav(
            "缺少 WAVE 文件标识".to_string(),
        ));
    }

    let mut fmt_header: Option<WavHeader> = None;
    let mut data_offset: Option<u64> = None;
    let mut data_size: Option<u32> = None;

    loop {
        let mut chunk_id = [0_u8; 4];
        match reader.read_exact(&mut chunk_id) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        }

        let chunk_size = read_u32_le(reader)?;
        let chunk_data_offset = reader.stream_position()?;

        match &chunk_id {
            b"fmt " => {
                // PCM 的 fmt chunk 至少 16 字节，扩展字段在读取基础头后跳过。
                if chunk_size < 16 {
                    return Err(WaveSculptorError::InvalidWav(
                        "fmt 数据块长度小于 16 字节".to_string(),
                    ));
                }

                let header = WavHeader {
                    audio_format: read_u16_le(reader)?,
                    channels: read_u16_le(reader)?,
                    sample_rate: read_u32_le(reader)?,
                    byte_rate: read_u32_le(reader)?,
                    block_align: read_u16_le(reader)?,
                    bits_per_sample: read_u16_le(reader)?,
                    data_size: 0,
                };
                fmt_header = Some(header);

                let extra_bytes = i64::from(chunk_size) - 16;
                if extra_bytes > 0 {
                    reader.seek(SeekFrom::Current(extra_bytes))?;
                }
            }
            b"data" => {
                data_offset = Some(chunk_data_offset);
                data_size = Some(chunk_size);
                // 先记录位置，继续扫描以兼容包含其它 chunk 的 WAV 文件。
                reader.seek(SeekFrom::Current(i64::from(chunk_size)))?;
            }
            _ => {
                reader.seek(SeekFrom::Current(i64::from(chunk_size)))?;
            }
        }

        // RIFF chunk 数据按偶数字节对齐，奇数字节长度后会有一个填充字节。
        if chunk_size % 2 != 0 {
            reader.seek(SeekFrom::Current(1))?;
        }
    }

    let mut header = fmt_header.ok_or(WaveSculptorError::MissingChunk("fmt "))?;
    let offset = data_offset.ok_or(WaveSculptorError::MissingChunk("data"))?;
    header.data_size = data_size.ok_or(WaveSculptorError::MissingChunk("data"))?;

    Ok((header, offset))
}

fn read_u16_le<R: Read>(reader: &mut R) -> Result<u16> {
    let mut bytes = [0_u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32_le<R: Read>(reader: &mut R) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{parse_header, parse_wav};

    fn build_test_wav(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let data_size = (samples.len() * 2) as u32;
        let byte_rate = sample_rate * u32::from(channels) * 2;
        let block_align = channels * 2;
        let file_size = 36 + data_size;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&file_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn parses_wav_header_fields() {
        let wav = build_test_wav(&[100, -100, 200, -200], 44_100, 2);
        let mut cursor = Cursor::new(wav);
        match parse_header(&mut cursor) {
            Ok(header) => {
                assert_eq!(header.audio_format, 1);
                assert_eq!(header.channels, 2);
                assert_eq!(header.sample_rate, 44_100);
                assert_eq!(header.byte_rate, 176_400);
                assert_eq!(header.block_align, 4);
                assert_eq!(header.bits_per_sample, 16);
                assert_eq!(header.data_size, 8);
            }
            Err(error) => panic!("header should parse: {error}"),
        }
    }

    #[test]
    fn parses_i16_samples() {
        let wav = build_test_wav(&[1, -2, 300, -400], 48_000, 1);
        let mut cursor = Cursor::new(wav);
        match parse_wav(&mut cursor) {
            Ok(audio) => {
                assert_eq!(audio.samples, vec![1, -2, 300, -400]);
                assert_eq!(audio.channels, 1);
                assert_eq!(audio.sample_rate, 48_000);
            }
            Err(error) => panic!("wav should parse: {error}"),
        }
    }
}
