import os
import subprocess
import urllib.request

MODELS_DIR = 'models'
DIRS = [MODELS_DIR, 'tmp/input', 'tmp/output', 'tmp/transcripts']

def create_dirs():
    for dir_path in DIRS:
        os.makedirs(dir_path, exist_ok=True)

FILES = [
    'https://hf-mirror.com/hexgrad/Kokoro-82M/resolve/main/kokoro-v0_19.onnx',
    'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin',
    'https://github.com/farshed/sage/releases/download/voices.json_v0.1/voices.json'
]

def download_files():
    for url in FILES:
        try:
            filename = os.path.join(MODELS_DIR, url.split("/")[-1])
            urllib.request.urlretrieve(url, filename)

            print(f"Downloaded: {filename}")
        
        except Exception as e:
            print(f"Error downloading {url}: {e}")

def compile_rust_project(project_path):
    try:
        if not os.path.isdir(project_path):
            print(f"Error: Project not found at '{project_path}'")
            return

        result = subprocess.run(['cargo', 'build', '--release'], cwd=project_path, capture_output=True, text=True, check=True)
        
        print("Build successful.")
        print(result.stdout)
    except subprocess.CalledProcessError as e:
        print(f"Build failed for {project_path}: {e}. Make sure you have rust installed.")
        print(e.stderr) 

create_dirs()
download_files()
compile_rust_project('kokoro')
compile_rust_project('whisper')
