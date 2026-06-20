use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::audio::buffer::AudioBuffer;
use crate::error::Result;

pub fn write_wav_file(path: &Path, buffer: &AudioBuffer) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_wav(&mut writer, buffer)?;
    writer.flush()?;
    Ok(())
}

pub fn write_wav_to_vec(buffer: &AudioBuffer) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_wav(&mut bytes, buffer)?;
    Ok(bytes)
}

pub fn write_wav<W: Write>(writer: &mut W, buffer: &AudioBuffer) -> Result<()> {
    // 写出最小 PCM WAV：RIFF 头、fmt chunk 和 data chunk。
    let data_size = (buffer.samples.len() * std::mem::size_of::<i16>()) as u32;
    let file_size = 36 + data_size;
    let byte_rate = buffer.sample_rate * u32::from(buffer.channels) * 2;
    let block_align = buffer.channels * 2;

    writer.write_all(b"RIFF")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&buffer.channels.to_le_bytes())?;
    writer.write_all(&buffer.sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&buffer.bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_size.to_le_bytes())?;

    // 内部采样就是 i16 PCM，所以直接按小端序输出。
    for sample in &buffer.samples {
        writer.write_all(&sample.to_le_bytes())?;
    }

    Ok(())
}
