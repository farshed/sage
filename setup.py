import os
import urllib.request

MODELS_DIR = './models'
FILES = [
    'https://hf-mirror.com/hexgrad/Kokoro-82M/resolve/main/kokoro-v0_19.onnx',
    'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin',
    
]

# create tmp dirs

def download_files():
    if not os.path.exists(MODELS_DIR):
        os.makedirs(MODELS_DIR)

    for url in FILES:
        try:
            filename = os.path.join(MODELS_DIR, url.split("/")[-1])
            urllib.request.urlretrieve(url, filename)

            print(f"Downloaded: {filename}")
        
        except Exception as e:
            print(f"Error downloading {url}: {e}")

download_files()
