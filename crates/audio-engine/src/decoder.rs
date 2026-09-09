use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use symphonia::core::{audio::{AudioBufferRef, SampleBuffer}, codecs::CODEC_TYPE_NULL, errors::Error, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint};

/// Decodes MP3, FLAC, and WAV into interleaved 32-bit float samples.
pub fn decode_file(path: impl AsRef<Path>) -> Result<Vec<f32>> {
    let path = path.as_ref();
    let source = File::open(path).with_context(|| format!("unable to open {}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format.default_track().context("file has no default audio track")?;
    if track.codec_params.codec == CODEC_TYPE_NULL {
        anyhow::bail!("file uses an unsupported audio codec");
    }
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;
    let mut samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => append_f32(&mut samples, decoded),
            Err(Error::DecodeError(_)) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(samples)
}

fn append_f32(samples: &mut Vec<f32>, decoded: AudioBufferRef<'_>) {
    let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
    buffer.copy_interleaved_ref(decoded);
    samples.extend_from_slice(buffer.samples());
}