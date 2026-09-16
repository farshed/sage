use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};
use std::{
    env,
    error::Error,
    fs,
    io::{self, Cursor, Read},
    path::Path,
};

use sage_protocol as protocol;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("Parakeet: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--worker" {
        let recognizer = load_model(Path::new(&args[1]))?;
        return serve(&recognizer);
    }
    if args.len() != 3 {
        return Err("Usage: parakeet --worker <model-directory> | parakeet <model-directory> <input.wav> <transcript.txt>".into());
    }

    let (sample_rate, samples) = read_audio(fs::File::open(&args[1])?)?;
    if samples.is_empty() {
        fs::write(&args[2], "")?;
        return Ok(());
    }

    let recognizer = load_model(Path::new(&args[0]))?;
    fs::write(&args[2], transcribe(&recognizer, sample_rate, &samples)?)?;
    Ok(())
}

fn load_model(model_dir: &Path) -> Result<OfflineRecognizer> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(model_file(model_dir, "encoder.int8.onnx")?),
        decoder: Some(model_file(model_dir, "decoder.int8.onnx")?),
        joiner: Some(model_file(model_dir, "joiner.int8.onnx")?),
    };
    config.model_config.tokens = Some(model_file(model_dir, "tokens.txt")?);
    config.model_config.model_type = Some("nemo_transducer".into());
    config.model_config.provider = Some("cpu".into());
    config.model_config.num_threads = 2;
    config.decoding_method = Some("greedy_search".into());

    OfflineRecognizer::create(&config)
        .ok_or_else(|| "Failed to load Parakeet TDT. Check the model files.".into())
}

fn transcribe(recognizer: &OfflineRecognizer, sample_rate: i32, samples: &[f32]) -> Result<String> {
    if samples.is_empty() {
        return Ok(String::new());
    }
    let stream = recognizer.create_stream();
    stream.accept_waveform(sample_rate, samples);
    recognizer.decode(&stream);
    let result = stream
        .get_result()
        .ok_or("No transcription result returned")?;
    Ok(result.text.trim().to_string())
}

fn serve(recognizer: &OfflineRecognizer) -> Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::BufWriter::new(io::stdout().lock());
    protocol::write_frame(&mut output, protocol::READY, &[])?;
    while let Some(bytes) = protocol::read_request(&mut input)? {
        let result = read_audio(Cursor::new(bytes))
            .and_then(|(rate, samples)| transcribe(recognizer, rate, &samples));
        match result {
            Ok(text) => {
                protocol::write_frame(&mut output, protocol::DATA, text.as_bytes())?;
                protocol::write_frame(&mut output, protocol::DONE, &[])?;
            }
            Err(error) => {
                protocol::write_frame(&mut output, protocol::ERROR, error.to_string().as_bytes())?
            }
        }
    }
    Ok(())
}

fn model_file(directory: &Path, name: &str) -> Result<String> {
    let path = directory.join(name);
    if !path.is_file() {
        return Err(format!(
            "Missing model file: {}. Run the setup script.",
            path.display()
        )
        .into());
    }
    Ok(path
        .to_str()
        .ok_or("Model path must be valid UTF-8")?
        .into())
}

fn read_audio(reader: impl Read) -> Result<(i32, Vec<f32>)> {
    let mut wav = hound::WavReader::new(reader)?;
    let spec = wav.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err("Expected a 16-bit PCM WAV recording".into());
    }
    let sample_rate = i32::try_from(spec.sample_rate)?;
    if spec.channels == 0 || sample_rate == 0 {
        return Err("Recording must have channels and a positive sample rate".into());
    }
    let samples = wav
        .samples::<i16>()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let channels = usize::from(spec.channels);
    if samples.len() % channels != 0 {
        return Err("Recording contains an incomplete audio frame".into());
    }
    let mono = samples
        .chunks_exact(channels)
        .map(|frame| {
            frame
                .iter()
                .map(|&sample| f32::from(sample) / 32768.0)
                .sum::<f32>()
                / f32::from(spec.channels)
        })
        .collect();
    Ok((sample_rate, mono))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn recording(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut buffer,
            hound::WavSpec {
                channels,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for &sample in samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        buffer.into_inner()
    }

    #[test]
    fn preserves_mono_samples_and_original_sample_rate() {
        for sample_rate in [16000, 44100, 48000] {
            let bytes = recording(1, sample_rate, &[0, 16384, -32768, 32767]);
            let (rate, samples) = read_audio(Cursor::new(bytes)).unwrap();
            assert_eq!(rate, sample_rate as i32);
            assert_eq!(samples, vec![0.0, 0.5, -1.0, 32767.0 / 32768.0]);
        }
    }

    #[test]
    fn mixes_stereo_without_changing_duration() {
        let bytes = recording(2, 48000, &[16384, 16384, 16384, -16384, -32768, 0]);
        let (rate, samples) = read_audio(Cursor::new(bytes)).unwrap();
        assert_eq!(rate, 48000);
        assert_eq!(samples, vec![0.5, 0.0, -0.5]);
    }

    #[test]
    fn accepts_empty_recordings() {
        let bytes = recording(1, 16000, &[]);
        assert!(read_audio(Cursor::new(bytes)).unwrap().1.is_empty());
    }

    #[test]
    fn rejects_invalid_audio() {
        assert!(read_audio(Cursor::new(b"not a WAV")).is_err());
        let mut bytes = recording(1, 16000, &[1, 2]);
        bytes.pop();
        assert!(read_audio(Cursor::new(bytes)).is_err());
    }
}
