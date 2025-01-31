mod onn;
mod tts;
mod utils;

use std::{fs::{self}, io::Write, path::Path};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::utils::wav::{write_audio_chunk, WavHeader};
use clap::Parser;
use tts::koko::TTSKoko;

#[derive(Parser, Debug)]
#[command(name = "kokoros")]
#[command(version = "0.1")]
#[command(author = "Lucas Jin")]
struct Cli {
    #[arg(short = 't', long = "text", value_name = "TEXT")]
    text: Option<String>,

    #[arg(
        short = 'l',
        long = "lan",
        value_name = "LANGUAGE",
        help = "https://github.com/espeak-ng/espeak-ng/blob/master/docs/languages.md"
    )]
    lan: Option<String>,

    #[arg(short = 'm', long = "model", value_name = "MODEL")]
    model: Option<String>,

    #[arg(short = 's', long = "style", value_name = "STYLE")]
    style: Option<String>,

    #[arg(long = "oai", value_name = "OpenAI server")]
    oai: bool,

    #[arg(long = "stream", help = "Enable streaming mode")]
    stream: bool,
}
async fn handle_streaming_mode(
    tts: &TTSKoko,
    lan: &str,
    style: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Use std::io::stdout() for sync writing
    let mut stdout = std::io::stdout();

    // Write WAV header first
    eprintln!("Entering streaming mode. Type text and press Enter. Use Ctrl+D to exit.");

    let header = WavHeader::new(1, 24000, 32);
    header.write_header(&mut stdout)?;
    stdout.flush()?;

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        // Process the line and get audio data
        match tts.tts_raw_audio(&line, lan, style) {
            Ok(raw_audio) => {
                // Write the raw audio samples directly
                write_audio_chunk(&mut stdout, &raw_audio)?;
                stdout.flush()?;
            }
            Err(e) => eprintln!("Error processing line: {}", e),
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let args = Cli::parse();

        // if users use `af_sky.4+af_nicho.3` as style name
        // then we blend it, with 0.4 af_sky + 0.3 af_nicho

        let model_path = args.model.unwrap_or_else(|| "models/kokoro-v0_19.onnx".to_string());
        let style = args.style.unwrap_or_else(|| "af_sky".to_string());
        let lan = args.lan.unwrap_or_else(|| { "en-us".to_string() });

        let tts = TTSKoko::new(&model_path);

        if args.stream {
            handle_streaming_mode(&tts, &lan, &style).await?;
            Ok(())
        } 
        else {
            let mut txt = args.text;
            if txt.is_none() {
                txt = Some(r#"
                    Hello, This is Kokoro, your remarkable AI TTS. It's a TTS model with merely 82 million parameters yet delivers incredible audio quality.
                    This is one of the top notch Rust based inference models, and I'm sure you'll love it. If you do, please give us a star. Thank you very much.
                    As the night falls, I wish you all a peaceful and restful sleep. May your dreams be filled with joy and happiness. Good night, and sweet dreams!
                "#.to_string());
            }
        
            if let Some(txt_path) = &txt {
                let path = Path::new(txt_path);
                if path.exists() && path.is_file() {
                    let file_content = fs::read_to_string(txt_path)?;
                    for (i, line) in file_content.lines().enumerate() {
                        let stripped_line = line.trim(); 
                        if !stripped_line.is_empty() {
                            let save_path = format!("tmp/output_{}.wav", i);
                            tts.tts(stripped_line, &lan, &style, &save_path)?;
                        }
                    }
                    return Ok(());
                }
            }
        
            if let Some(text) = txt {
                let save_path = "tmp/output.wav";
                tts.tts(&text, &lan, &style, &save_path)?;
            }
            Ok(())
        }
    })
}
