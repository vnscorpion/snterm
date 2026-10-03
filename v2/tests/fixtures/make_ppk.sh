#!/bin/sh
# Tạo key PPK mẫu bằng puttygen để kiểm thử bộ đọc PPK (không phải key thật).
set -e
cd "$(dirname "$0")/ppk"
puttygen -t ed25519 -q -o ed25519-v3-plain.ppk --ppk-param version=3 -C "ed25519 plain" --new-passphrase /dev/null
puttygen ed25519-v3-plain.ppk -O public-openssh -o ed25519-v3-plain.pub
printf 'secret123' > /tmp/pp.txt
puttygen -t ed25519 -q -o ed25519-v3-pass.ppk --ppk-param version=3 -C "ed25519 pass" --new-passphrase /tmp/pp.txt
puttygen ed25519-v3-pass.ppk --old-passphrase /tmp/pp.txt -O public-openssh -o ed25519-v3-pass.pub
puttygen -t rsa -b 2048 -q -o rsa-v3-plain.ppk --ppk-param version=3 -C "rsa plain" --new-passphrase /dev/null
puttygen rsa-v3-plain.ppk -O public-openssh -o rsa-v3-plain.pub
puttygen -t rsa -b 2048 -q -o rsa-v2-pass.ppk --ppk-param version=2 -C "rsa v2 pass" --new-passphrase /tmp/pp.txt
puttygen rsa-v2-pass.ppk --old-passphrase /tmp/pp.txt -O public-openssh -o rsa-v2-pass.pub
puttygen -t ecdsa -b 256 -q -o ecdsa-v3-plain.ppk --ppk-param version=3 -C "ecdsa plain" --new-passphrase /dev/null
puttygen ecdsa-v3-plain.ppk -O public-openssh -o ecdsa-v3-plain.pub
rm -f /tmp/pp.txt
ls -la
