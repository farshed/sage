import os
import urllib.request

MODELS_DIR = './models'
FILES = [
    
]

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
