use clap::Parser;
use std::fs::File;
use std::io::{Result, Write};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

// Todo: optimize
// Stream?
// Quantize?

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'm', long = "model", value_name = "MODEL")]
    model: Option<String>,

    #[arg(
        short = 'l',
        long = "lang",
        value_name = "LANGUAGE",
        default_value = "en",
        help = "https://github.com/openai/whisper/blob/main/whisper/tokenizer.py#L10"
    )]
    lang: String,

    #[arg(short = 'i', long = "file_id", value_name = "FILE_ID")]
    id: Option<String>,
}

fn main() {
    let args = Args::parse();
    let model_path = args.model.expect("Please specify model path");
    let language = args.lang;
    let id = args.id.expect("Please specify file id");
    let wav_path = format!("./tmp/input/{}.wav", id);

    let samples: Vec<i16> = hound::WavReader::open(wav_path)
        .unwrap()
        .into_samples::<i16>()
        .map(|x| x.unwrap())
        .collect();

    let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
        .expect("failed to load model");

    let mut state = ctx.create_state().expect("failed to create state");
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    params.set_language(Some(&language));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_token_timestamps(false);

    let mut inter_samples = vec![Default::default(); samples.len()];
    whisper_rs::convert_integer_to_float_audio(&samples, &mut inter_samples)
        .expect("failed to convert audio data");

    let samples = whisper_rs::convert_stereo_to_mono_audio(&inter_samples)
        .expect("failed to convert audio data");

    state
        .full(params, &samples[..])
        .expect("failed to run model");

    let num_segments = state
        .full_n_segments()
        .expect("failed to get number of segments");

    let mut output = String::new();

    for i in 0..num_segments {
        let segment = state
            .full_get_segment_text(i)
            .expect("failed to get segment");
        output.push_str(&segment);
    }

    write_to_file(&id, &output).unwrap();
}

fn write_to_file(id: &str, data: &str) -> Result<()> {
    let path = format!("./tmp/transcripts/{}.txt", id);
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}
