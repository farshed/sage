use std::env::args;
use std::fs::File;
use std::io::{Result, Write};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

// Todo: optimize?
// Speed up the audio
// Cut blank parts out

const MODEL_PATH: &'static str = "./models/ggml-base.en.bin";

fn main() {
    let id = args().nth(1).expect("Please specify file id");
    let wav_path = format!("./tmp/input/{}.wav", id);
    let language = "en";

    let samples: Vec<i16> = hound::WavReader::open(wav_path)
        .unwrap()
        .into_samples::<i16>()
        .map(|x| x.unwrap())
        .collect();

    let ctx = WhisperContext::new_with_params(&MODEL_PATH, WhisperContextParameters::default())
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
