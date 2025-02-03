@echo off

:: create .env
echo. > .env

:: create directories

mkdir models tmp
mkdir tmp\input tmp\output tmp\transcripts

:: download models

echo Downloading models...
cd models
curl -O https://hf-mirror.com/hexgrad/Kokoro-82M/resolve/main/kokoro-v0_19.onnx
curl -O https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin
curl -O https://github.com/farshed/sage/releases/download/voices.json_v0.1/voices.json

:: compile whisper and kokoro

cd ..\whisper
cargo build --release
cd ..\kokoro
cargo build --release