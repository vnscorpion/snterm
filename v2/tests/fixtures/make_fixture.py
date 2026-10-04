#!/usr/bin/env python3
"""Tạo fixture .snterm độc lập (Python cryptography) theo đúng thuật toán v1 C#:
PBKDF2-SHA256 600000 vòng, AES-256-GCM, AAD = id (check: "SNTERM-CHECK"), plaintext check "SNTERM-OK".
Dùng để kiểm tra chéo bộ giải mã Rust mà không cần .NET."""
import base64, json, os, uuid, datetime
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

PASSWORD = "StrongPassword@123"
salt = os.urandom(16)
key = PBKDF2HMAC(hashes.SHA256(), 32, salt, 600000).derive(PASSWORD.encode())

def block(plain: bytes, aad: bytes):
    nonce = os.urandom(12)
    out = AESGCM(key).encrypt(nonce, plain, aad)
    return {"nonce": base64.b64encode(nonce).decode(),
            "cipherText": base64.b64encode(out[:-16]).decode(),
            "tag": base64.b64encode(out[-16:]).decode()}

sid = str(uuid.uuid4())
secret = json.dumps({"password": "VMPassword_456!", "passphrase": None, "keyFileContent": None}, ensure_ascii=False).encode()
doc = {
  "format": "snterm-sessions", "version": 1,
  "exportedAt": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f0Z"),
  "appVersion": "1.0.0",
  "protection": {"kdf": "PBKDF2-SHA256", "iterations": 600000, "salt": base64.b64encode(salt).decode(),
                 "check": block(b"SNTERM-OK", b"SNTERM-CHECK")},
  "sessions": [{"id": sid, "name": "Máy chủ fixture Ắ Ằ Ợ", "group": "Dev", "host": "10.0.0.5", "port": 22,
                "username": "root", "keyFileName": None, "secrets": block(secret, sid.encode())}]
}
with open(os.path.join(os.path.dirname(__file__), "v1-protected.snterm"), "w", encoding="utf-8") as f:
    json.dump(doc, f, ensure_ascii=False, indent=2)
print("ok", sid)
