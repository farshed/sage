use std::{io::{Read, Write}, time::Duration};

fn frame(kind: u8, data: &[u8]) {
    let mut output = std::io::stdout().lock();
    output.write_all(&[kind]).unwrap();
    output.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
    output.write_all(data).unwrap();
    output.flush().unwrap();
}

fn main() {
    frame(0, &[]);
    let mode = std::env::args().nth(1).unwrap_or_default();
    let mut input = std::io::stdin().lock();
    loop {
        let mut header = [0; 5];
        if input.read_exact(&mut header).is_err() { return; }
        let mut bytes = vec![0; u32::from_le_bytes(header[1..].try_into().unwrap()) as usize];
        input.read_exact(&mut bytes).unwrap();
        match bytes.as_slice() {
            b"crash" => std::process::exit(1),
            b"hang" => std::thread::sleep(Duration::from_secs(10)),
            b"bad" => { frame(3, b"Invalid input"); continue; }
            b"slow" => std::thread::sleep(Duration::from_millis(100)),
            _ => {},
        }
        match mode.as_str() {
            "asr" => frame(1, b"Hello"),
            "tts" => for _ in 0..20 { frame(1, &0.25_f32.to_le_bytes()); },
            _ => { frame(1, std::process::id().to_string().as_bytes()); frame(1, &bytes); },
        }
        frame(2, &[]);
    }
}
