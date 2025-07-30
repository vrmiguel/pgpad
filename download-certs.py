#!/usr/bin/env python3

# Download official certificate bundles from RDS, Google CloudSQL, and Azure, validating their integrity with hash checks.

import sys
import urllib.request
import urllib.error
from pathlib import Path
import hashlib

CERTIFICATE_BUNDLES = {
    "aws-rds": {
        "url": "https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem",
        "filename": "aws-rds-global-bundle.pem",
        "expected_sha256": "e5bb2084ccf45087bda1c9bffdea0eb15ee67f0b91646106e466714f9de3c7e3",
    },
    "azure-database": {
        "url": "https://www.digicert.com/CACerts/BaltimoreCyberTrustRoot.crt.pem",
        "filename": "azure-baltimore-root.pem", 
        "expected_sha256": "285963b0968a2204019db351ef5d1c97d732f1c4de00d3ae035e8987c954f945",
    }
}

def download_certificate_bundle(url: str, output_path: Path) -> tuple[bool, bytes]:
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            if response.status != 200:
                print(f"Error: HTTP {response.status} when downloading {url}")
                return False, b""
            content = response.read()
            
        with open(output_path, 'wb') as f:
            f.write(content)
            
        return True, content
        
    except (urllib.error.URLError, IOError) as e:
        print(f"Error downloading {url}: {e}")
        return False, b""

def verify_hash(content: bytes, expected_hash: str) -> bool:
    actual_hash = hashlib.sha256(content).hexdigest()
    
    if actual_hash.lower() != expected_hash.lower():
        print(f"Hash verification failed for content")
        print(f"Expected: {expected_hash}")
        print(f"Actual:   {actual_hash}")
        return False
    
    return True

def main():
    current_dir = Path.cwd()
    certs_dir = current_dir / "certs"
    certs_dir.mkdir(parents=True, exist_ok=True)
    
    success_count = 0
    
    for bundle_key, bundle in CERTIFICATE_BUNDLES.items():
        bundle_path = certs_dir / bundle["filename"]
        
        success, content = download_certificate_bundle(bundle["url"], bundle_path)
        if not success:
            continue
        
        if not verify_hash(content, bundle["expected_sha256"]):
            bundle_path.unlink(missing_ok=True)
            continue
        
        print(f"Downloaded: {bundle_path}")
        success_count += 1
    
    if success_count == 0:
        print("No certificate bundles were successfully downloaded.")
        sys.exit(1)
    
    print(f"Successfully downloaded {success_count} certificate bundles to {certs_dir}")

if __name__ == "__main__":
    main()