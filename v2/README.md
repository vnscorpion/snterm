# SN Term v2 (Rust + Tauri 2)

Bản viết lại của SN Term theo kế hoạch `SN Term v2 - Phan 1 - Chuyen ngon ngu.md`, **phương án A**: lõi Rust, một WebView2 duy nhất, giao diện Svelte 5 + xterm.js. Bố cục và toàn bộ tính năng giữ như v1; dữ liệu dùng chung với v1 (`%APPDATA%\SNTerm`).

## Cấu trúc

```
v2/
├─ Cargo.toml                      # workspace
├─ crates/core/                    # snterm-core: lưu trữ, mã hóa, SSH/SFTP (không phụ thuộc UI)
│  └─ src/ paths.rs, error.rs, store/, crypto/, import/, ssh/
├─ crates/app-tauri/
│  ├─ src-tauri/                   # app Tauri: lệnh (commands/), trạng thái, tauri.conf.json, icons
│  ├─ src/                         # giao diện Svelte: App.svelte, components/, stores/, lib/
│  ├─ public/                      # font JetBrains Mono, icon hệ điều hành
│  └─ scripts/screenshot.mjs       # chụp màn hình giao diện ở chế độ mock (trình duyệt)
└─ tests/fixtures/                 # file .snterm tạo độc lập bằng Python, key PPK mẫu (puttygen), sshd thử nghiệm
```

## Build trên Windows

Yêu cầu: Rust stable (rustup, toolchain msvc), Node.js 22+, Visual Studio Build Tools (C++), WebView2 Runtime (có sẵn trên Windows 10/11). Không cần NASM/CMake: `russh` dùng backend `ring`.

```powershell
cd v2/crates/app-tauri
npm install
npm run tauri build          # → src-tauri/target/release/SNTerm.exe (portable) + bộ cài NSIS trong target/release/bundle/nsis
npm run tauri dev            # chạy thử có hot-reload
```

Bản portable chỉ cần một file `snterm.exe` (tài nguyên web đã nhúng trong exe). Bộ cài NSIS tự tải WebView2 nếu máy chưa có.

Nếu build thẳng bằng `cargo build --release` (không qua `npm run tauri build`), phải thêm `--features tauri/custom-protocol`, nếu không exe sẽ đi tìm máy chủ dev `localhost:1420` và hiện "can't reach this page". Cross-compile thử từ Linux: `rustup target add x86_64-pc-windows-gnu`, cài `gcc-mingw-w64-x86-64`, rồi `cargo build --release --target x86_64-pc-windows-gnu -p snterm --features tauri/custom-protocol` và chép `WebView2Loader.dll` (trong `target/.../build/webview2-com-sys-*/out/x64/`) cạnh exe.

## Kiểm thử

```bash
# Lõi Rust (unit test; đọc fixture .snterm và PPK)
cargo test -p snterm-core

# Tích hợp SSH/SFTP với sshd cục bộ (Linux/WSL): cổng 2222, user sntest / Test@1234
#   xem tests/sshd/sshd_config; tạo key: ssh-keygen vào tests/sshd/client_ed25519, client_rsa_pass (passphrase keypass123)
/usr/sbin/sshd -f tests/sshd/sshd_config
cargo test -p snterm-core --test ssh_integration

# Giao diện
cd crates/app-tauri
npm test                     # vitest (tìm nhanh bằng bàn phím, định dạng)
npm run check                # svelte-check
node scripts/screenshot.mjs  # chụp màn hình mock → scripts/out/*.png (cần Chromium của Playwright)
```

Tạo lại fixture: `python3 tests/fixtures/make_fixture.py` (cần `cryptography`), `tests/fixtures/make_ppk.sh` (cần `puttygen`).

## Tương thích dữ liệu với v1

| File | Ghi chú |
|---|---|
| `sessions.json`, `settings.json`, `known_hosts.json` | Cùng tên trường (PascalCase), cùng định dạng ngày `.NET`; trường lạ được giữ nguyên khi ghi lại |
| Mật khẩu đã lưu | DPAPI CurrentUser, entropy `SNTerm.DPAPI.Entropy.v1` như v1 → v2 đọc được mật khẩu v1 và ngược lại |
| `.snterm` | AES-256-GCM + PBKDF2-SHA256 600.000 vòng, AAD = id; test chéo với fixture tạo bằng Python |
| `.mxtsessions` / `MobaXterm.ini` | Cùng quy tắc đọc như v1 |
| Key `.ppk` | v2 và v3 (Argon2), qua bộ đọc của `ssh-key`; kiểm với key do `puttygen` tạo |

Biến môi trường `SNTERM_DATA_DIR` (chỉ để kiểm thử) chuyển thư mục dữ liệu sang chỗ khác.

## Khác biệt kỹ thuật so với v1

- Một kết nối TCP cho mỗi tab: SFTP và lệnh monitor chạy trên kênh phụ của cùng phiên SSH.
- Output terminal đi qua `tauri::ipc::Channel` dạng bytes thô, gom mỗi 16 ms / 64 KB.
- Monitor máy chủ chỉ chạy khi tab đang được xem (tạm dừng khi ẩn).
- Phát hiện mất kết nối: keepalive của `russh` (3 lần × KeepAliveSeconds) + thăm dò nhanh sau lệnh `reboot`/`shutdown`.
- Hộp thoại host key / mật khẩu do lõi yêu cầu qua sự kiện `dialog:request`, xếp hàng lần lượt như v1.
- Chừa sẵn cho Phần 2 (đồng bộ): trường lạ trong `sessions.json`/`settings.json` được giữ nguyên; `crypto/snterm_file.rs` tách riêng hàm mã hóa/giải mã khối.

## Trạng thái (Phần 1, phương án A)

Đã làm và kiểm thử tự động trên Linux (xem `cargo test`, `npm test`, ảnh chụp `scripts/out`):
- Giai đoạn 0–6 theo kế hoạch: khung dự án, dữ liệu tương thích v1, SSH + terminal + thanh tab, clipboard/phím tắt, SFTP, Export/Import, Cài đặt/log/đóng app.
- Tích hợp SSH/SFTP chạy thật với `sshd` cục bộ: mật khẩu, key ed25519, key RSA có passphrase, host key, pty resize, tiếng Việt, exec monitor, upload/download/chmod/rename/xóa đệ quy.
- Crate `snterm-core` và app Tauri type-check sạch cho target Windows (`x86_64-pc-windows-gnu`); DPAPI dùng `windows` crate.

Cần kiểm tra **[Tay]** trên Windows thật (chưa có máy Windows trong môi trường phát triển):
1. `npm run tauri build` ra exe; mở app trên máy đang có v1 → thấy đủ VM, nhấp đúp vào thẳng (mật khẩu DPAPI của v1 giải mã được).
2. Gõ tiếng Việt bằng Unikey/EVKey trong `nano`; `htop`, `vim` hiển thị đúng; kéo cửa sổ thì terminal dàn lại.
3. Kéo thả file từ Explorer vào SFTP; mở file bằng Notepad rồi lưu → file trên VM đổi.
4. Nhấp đúp file `.snterm` mở app và hiện hộp thoại Import.
5. Đo RAM/khởi động so với v1 theo bảng mục 1 của kế hoạch.
