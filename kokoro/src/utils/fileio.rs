use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use serde_json::Value;

pub fn download_file_from_url(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let client = Client::new();
    let mut resp = client.get(url).send()?;

    if resp.status().is_success() {
        let total_size = resp.content_length().unwrap_or(0);

        eprintln!("total size: {}", total_size);

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut file = File::create(path)?;
        let mut buffer = [0; 8192]; // 8KB buffer
        let mut downloaded = 0;

        loop {
            let bytes_read = resp.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])?;
            downloaded += bytes_read as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download completed");
        Ok(())
    } else {
        Err(format!("Failed to download file: {}", resp.status()).into())
    }
}

pub fn load_json_file(path: &str) -> Result<Value, String> {
    let file = File::open(path);
    if file.is_err() {
        return Err(format!("failed to open file: {}", file.err().unwrap()));
    }

    let mut data = String::new();
    file.unwrap()
        .read_to_string(&mut data)
        .map_err(|e| e.to_string())?;
    let json_value: Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    Ok(json_value)
}
