# SN Term v2 — Phần 1: Chuyển đổi ngôn ngữ (chạy nhẹ hơn, giữ bố cục)

> Viết lại **SN Term** bằng ngôn ngữ khác để **chạy nhẹ hơn** (ít RAM, khởi động nhanh, không cần cài .NET) và **giữ nguyên bố cục, toàn bộ tính năng** của bản v1 (C# / WPF).
> Tính năng **Đồng bộ danh sách VM** nằm ở tài liệu riêng `SN Term v2 - Phan 2 - Dong bo VM.md`; tài liệu này chỉ ghi chỗ cần "chừa sẵn" cho nó (mục 10).
>
> Tài liệu này tiếp nối `SN Term.md` (v1). Những gì v1 đã mô tả (bố cục, quy tắc form, định dạng file, thông báo tiếng Việt…) **vẫn là chuẩn**; ở đây chỉ ghi phần **khác đi**.

---

## 0. Hướng dẫn cho AI/CLI thực hiện kế hoạch này

1. Đọc `SN Term.md` (v1) trước, rồi đọc file này. Mục nào file này không nhắc tới thì làm **y như v1**.
2. Mã nguồn v1 trong `src/SNTerm/` là **tài liệu tham chiếu hành vi**: khi phân vân, mở file C# tương ứng (bảng ánh xạ mục 4.3) và làm cho giống.
3. **Chọn phương án trước khi code** (mục 3): **A** (Rust + Tauri, một WebView duy nhất) hoặc **B** (Rust + giao diện native, **không WebView**). Người dùng quyết định; nếu chưa có quyết định thì làm **Giai đoạn 0 spike** của cả hai (mục 11) rồi hỏi.
4. Làm **tuần tự từng giai đoạn** (mục 11). Sau mỗi giai đoạn: `cargo build --release` không lỗi, `cargo test` (và `npm test` nếu là phương án A) qua, rồi `git commit` với thông điệp `v2 Giai đoạn N: <tóm tắt>`.
5. **Không đổi bố cục** cửa sổ chính: thanh công cụ trên cùng, cột trái 2 tab Sessions/SFTP, thanh chia kéo được, cột phải thanh tab ngang + terminal, thanh trạng thái dưới cùng. Màu lấy đúng mã màu trong `Services/ThemeManager.cs`.
6. **Tương thích dữ liệu**: v2 đọc/ghi **cùng thư mục** `%APPDATA%\SNTerm\` và **cùng định dạng** `sessions.json`, `settings.json`, `known_hosts.json`, file `.snterm` của v1. Cài v2 đè lên v1 phải thấy đủ VM, nhấp đúp vào thẳng. Quay lại v1 vẫn chạy được.
7. Mọi chữ hiển thị bằng **tiếng Việt có dấu** (kèm bản tiếng Anh như v1). Tên biến, hàm, file bằng tiếng Anh.
8. Không ghi mật khẩu, passphrase ra log hay file dạng chữ thường.
9. Mọi thao tác mạng chạy bất đồng bộ (tokio); không bao giờ chặn luồng giao diện.
10. Người dùng không phải lập trình viên: báo cáo ngắn gọn, nói rõ cần họ làm gì, liệt kê các mục **[Tay]** cần tự bấm thử.

---

## 1. Mục tiêu và chỉ tiêu "nhẹ hơn"

| Chỉ tiêu | v1 (C# / WPF) | **A: Tauri** (1 WebView) | **B: Native** (không WebView) | Cách đo |
|---|---|---|---|---|
| Yêu cầu cài trước | .NET 10 Desktop Runtime + WebView2 Runtime | Chỉ WebView2 Runtime (có sẵn Win 10/11) | **Không cần gì** | — |
| File chạy | ~7 MB exe + thư mục `wwwroot` | 1 exe ≤ 10 MB | 1 exe ≤ 12 MB | Kích thước file |
| RAM mở app, 0 tab | ~120–150 MB | ≤ 90 MB | **≤ 50 MB** | Task Manager, tổng các tiến trình của app |
| RAM thêm mỗi tab | ~50–80 MB (một WebView2/tab) | ≤ 8 MB | **≤ 3 MB** | Mở 10 tab so với 1 tab |
| Khởi động tới khi thấy danh sách VM | ~1.5–3 s | ≤ 0.7 s | **≤ 0.3 s** | Bấm giờ |
| Kết nối TCP mỗi tab | 2 (SSH + SFTP riêng) | 1 | 1 | `netstat` |
| CPU khi để yên | ~0–1 % | 0 % | 0 % | Task Manager |
| Công sức so với A | — | 1× | **≈ 1.7–2×** (tự dựng widget terminal + giao diện không HTML) | — |

Số v1 là ước lượng; **Giai đoạn 9** đo lại cả hai bản trên cùng máy và ghi vào README.

---

## 2. Phạm vi

**Giữ nguyên từ v1 (phải có đủ):**
- Quản lý VM: thêm/sửa/nhân bản/xóa, nhóm, đổi tên/chuyển nhóm, tìm kiếm, nhóm "Gần đây", chọn nhiều, phím tắt, mật khẩu DPAPI, key file + passphrase, ⚠ thiếu key, icon hệ điều hành theo thông tin máy chủ.
- SSH: vào thẳng khi đã lưu mật khẩu, hỏi khi cần, host key, keyboard-interactive, mở nhiều VM song song (`MaxParallelConnects`), hàng đợi hộp thoại, mất kết nối → **R / Enter kết nối lại**, lệnh thu thập thông tin máy chủ (OS, uptime, RAM, đĩa, mạng).
- Terminal: bôi đen là copy, chuột phải dán/menu, xác nhận dán nhiều dòng, tiếng Việt (Unikey/EVKey), phím tắt `Ctrl+Shift+C/V/W`, `Ctrl+Tab`, `Alt+1..9`, `Ctrl+=/-/0`, font JetBrains Mono, theme sáng/tối, hiển thị đúng `vim`, `htop`, `nano`, `top`.
- Thanh tab: chấm trạng thái, đánh số tab trùng tên, cuộn ngang, kéo thả sắp xếp, chuột giữa đóng, menu chuột phải, giữ nội dung khi chuyển tab.
- SFTP: tự chuyển sang SFTP khi đăng nhập, đổi theo tab, duyệt, lên/làm mới/home, gõ đường dẫn, tạo thư mục, đổi tên, xóa, **chmod đệ quy**, **mở file bằng trình soạn thảo rồi tự upload lại**, upload/download có tiến trình, kéo thả từ Explorer, file ẩn, **tìm nhanh bằng bàn phím**.
- Export/Import `.snterm` (AES-256-GCM, PBKDF2 600.000 vòng, AAD = id), xử lý trùng, **Import MobaXterm**, kéo thả file vào cửa sổ, mở `.snterm` bằng nhấp đúp.
- Cài đặt (font, cỡ chữ, ngôn ngữ vi/en, theme, copy-on-select, chuột phải, scrollback, keepalive, file ẩn, trình soạn thảo), nhớ cửa sổ và độ rộng cột trái.
- Sao lưu tự động, khôi phục file hỏng, log, bắt lỗi toàn cục, hỏi khi đóng còn tab kết nối.

**Ngoài phạm vi phần này:** Đồng bộ VM (Phần 2); RDP, VNC, Telnet, Serial, X11, tunnel, macro, multi-exec, kéo file từ SFTP ra Explorer.

---

## 3. Hai phương án công nghệ

### 3.1 Phương án A — Rust + Tauri 2 (một WebView2 duy nhất)

| Thành phần | v1 | **A** |
|---|---|---|
| Ngôn ngữ lõi | C# / .NET 10 | **Rust** (stable, edition 2021) |
| Khung ứng dụng | WPF | **Tauri 2.x**: một cửa sổ, **một WebView2** cho toàn bộ giao diện |
| Giao diện | XAML | **HTML/CSS + Svelte 5 + TypeScript** (Vite) |
| Terminal | xterm.js trong WebView2 riêng mỗi tab | **xterm.js 6** + `addon-fit`, `addon-unicode11`, **`addon-webgl`**; nhiều `Terminal` trong cùng trang, ẩn/hiện bằng CSS |
| Clipboard, hộp thoại, mở app ngoài, log, trạng thái cửa sổ, một instance | WPF | plugin Tauri `clipboard-manager`, `dialog`, `opener`, `log`, `window-state`, `single-instance` |

**Ưu:** dùng lại xterm.js (không phải viết terminal), giao diện HTML dễ làm giống v1, bộ gõ tiếng Việt cùng engine WebView2 như v1 nên hành vi đã biết. **Nhược:** vẫn phụ thuộc WebView2 Runtime và vẫn tốn ~60–80 MB cho tiến trình trình duyệt (nhưng chỉ **một** lần, không nhân theo tab).

### 3.2 Phương án B — Rust + giao diện native, **không WebView**

| Thành phần | **B** | Ghi chú |
|---|---|---|
| Ngôn ngữ lõi | **Rust** | Giống A, dùng chung toàn bộ phần lõi (mục 3.3) |
| Giao diện | **`egui` / `eframe`** (winit + backend `glow` OpenGL; fallback phần mềm khi không có GPU) | Immediate-mode, nhẹ, có sẵn list, tab, modal, splitter tự vẽ. IME qua winit (`Event::Ime`) |
| Terminal | **`alacritty_terminal`** (bộ giả lập VT của Alacritty: parser, grid, màu, scrollback, selection) + **widget vẽ tự viết** bằng `egui::Painter` | VT100/xterm-256color, `vim`/`htop` chính xác vì dùng lõi Alacritty; phần tự viết là **vẽ chữ, con trỏ, vùng chọn, chuột, bảng phím → chuỗi escape** |
| Font | JetBrains Mono (terminal) + Segoe UI nạp từ hệ thống cho giao diện; font fallback cho emoji/CJK (`Segoe UI Emoji`, `Segoe UI Symbol`) | egui không shaping phức tạp, nhưng tiếng Việt là Unicode dựng sẵn (NFC) nên hiển thị đúng; icon 📁📄 thay bằng ảnh PNG nhỏ |
| Clipboard, hộp thoại, kéo thả, mở app ngoài | `arboard`, `rfd`, winit `DroppedFile`, `open` | |
| Phương án B2 (thay egui) | **Slint** | Có IME Windows, renderer phần mềm sẵn, UI khai báo; nhưng widget terminal phải vẽ qua `Image` từng frame → chọn egui làm mặc định của B |

**Ưu:** nhẹ nhất, không phụ thuộc bất kỳ runtime nào, khởi động tức thì, chạy tốt cả máy yếu/Windows cũ (Win 7/8 nếu cần). **Nhược:** công sức ≈ gấp đôi A; rủi ro kỹ thuật ở (1) **bộ gõ tiếng Việt** với winit (Unikey/EVKey chủ yếu bơm phím qua `SendInput` nên hoạt động với mọi app nhận `WM_CHAR`; chế độ "gửi Unicode + backspace" của EVKey cần kiểm tra), (2) **GPU**: qua Remote Desktop hoặc máy ảo không có OpenGL → cần fallback phần mềm (egui + `softbuffer`/`tiny-skia`), (3) chi tiết terminal (cuộn mượt, chọn theo từ, URL nhấp được, chữ đôi rộng/emoji) phải tự làm.

### 3.3 Phần lõi dùng chung cho cả A và B (Rust)

| Thành phần | Crate | Ghi chú |
|---|---|---|
| SSH | **`russh`** | password / publickey / keyboard-interactive, nhiều kênh trên một phiên, `window_change`, keepalive |
| SFTP | **`russh-sftp`** | Kênh phụ của cùng phiên SSH → 1 kết nối TCP/tab |
| Đọc key | **`ssh-key`** (OpenSSH, PKCS#8, PEM, passphrase) + **`crypto/ppk.rs` tự viết** (PPK v2/v3, v3 dùng `argon2`) | |
| DPAPI | crate **`windows`** (`CryptProtectData`/`CryptUnprotectData`), **giữ nguyên entropy** `SNTerm.DPAPI.Entropy.v1` | Để đọc `sessions.json` của v1 |
| `.snterm` | `aes-gcm`, `pbkdf2`, `sha2`, `zeroize` | Byte-compatible v1 |
| JSON | `serde`, `serde_json` (`#[serde(default)]`, bỏ qua trường lạ) | Tên trường giữ **y hệt** v1 |
| Async | `tokio` | |
| Theo dõi file (mở bằng trình soạn thảo) | `notify` | |
| Test | `cargo test`; A thêm `vitest` | |

**Vì sao không chọn cách khác:** giữ C# thì vẫn cần .NET; Go + Wails (dự phòng cho A nếu `russh` trục trặc) exe to hơn và Wails v3 chưa ổn định bằng Tauri 2; Electron nặng hơn v1; C++/Qt với QTermWidget không chạy tốt trên Windows (cần pty) và Qt nặng ~30 MB DLL.

### 3.4 Khuyến nghị
- Muốn **nhanh có bản dùng được, ít rủi ro**: chọn **A**. Vẫn đạt mục tiêu chính (không .NET, 1 WebView thay vì N, exe một file).
- Muốn **không đụng WebView, nhẹ tối đa, chạy mọi máy** và chấp nhận công sức gấp đôi: chọn **B**.
- Cả hai dùng **chung lõi Rust** (mục 3.3), nên có thể làm **A trước, B sau** mà không bỏ phí phần SSH/SFTP/mã hóa/lưu trữ (≈ 60 % mã nguồn). Đây là lộ trình an toàn nếu chưa chắc.

---

## 4. Kiến trúc và cấu trúc thư mục

### 4.1 Mô hình (chung; khác nhau ở lớp "UI")

```
┌──────────────── SNTerm.exe (Rust, tokio) ──────────────────┐
│ core/   store (sessions, settings, known_hosts, backups)    │
│         crypto (dpapi, snterm_file, ppk)                    │
│         ssh (connection, auth, host_key, shell, sftp, mon)  │
│         import (mobaxterm)   error (ErrorTranslator)        │
│         sync (chừa sẵn — Phần 2)                            │
├─────────────────────────────────────────────────────────────┤
│ UI  A: tauri commands + Channel + emit  ⇄  Svelte + xterm.js│
│     B: egui app state  ⇄  widget terminal (alacritty_terminal)│
└─────────────────────────────────────────────────────────────┘
```

Quy tắc: **`core/` không biết gì về UI** (không `tauri::`, không `egui::`), giao tiếp qua kênh `tokio::sync::mpsc` và các kiểu dữ liệu thuần (`CoreEvent`, `CoreCommand`). Nhờ vậy A và B hoán đổi được.

### 4.2 Thư mục

```
SNTerm/
├─ SN Term.md                                  // kế hoạch v1 (giữ)
├─ SN Term v2 - Phan 1 - Chuyen ngon ngu.md    // file này
├─ SN Term v2 - Phan 2 - Dong bo VM.md
├─ src/SNTerm/                                 // mã v1 C#, giữ để tham chiếu tới khi v2 phát hành
└─ v2/
   ├─ Cargo.toml (workspace)
   ├─ crates/core/            // mục 4.1, dùng chung
   │  └─ src/ lib.rs, paths.rs, store/, crypto/, ssh/, import/, error.rs, sync/ (trống, Phần 2)
   ├─ crates/app-tauri/       // phương án A
   │  ├─ src-tauri/ (main.rs, commands/*, tauri.conf.json, icons/)
   │  ├─ src/ (Svelte: App.svelte, components/, stores/, lib/{ipc.ts, i18n/, theme.css})
   │  ├─ public/fonts/, public/os/
   │  └─ package.json, vite.config.ts
   ├─ crates/app-native/      // phương án B
   │  └─ src/ main.rs, app.rs (layout), theme.rs, i18n.rs, widgets/{session_list, tab_bar, terminal, sftp_panel, dialogs}.rs
   └─ tests/fixtures/         // sessions.json, settings.json, .snterm, .mxtsessions do v1 tạo thật (không chứa bí mật thật)
```

### 4.3 Ánh xạ v1 → v2

| v1 (C#) | v2 |
|---|---|
| `Models/*.cs` | `core/store/model.rs`, kiểu TS trong `lib/ipc.ts` (A) |
| `Services/SessionStore.cs`, `SettingsStore.cs`, `KnownHostsStore.cs`, `AppPaths.cs` | `core/store/*`, `core/paths.rs` |
| `Services/SecretProtector.cs` | `core/crypto/dpapi.rs` |
| `Services/SessionExporter.cs`, `SessionImporter.cs`, `Models/ExportFile.cs` | `core/crypto/snterm_file.rs`, `core/store/import_export.rs` |
| `Services/MobaXtermImporter.cs` | `core/import/mobaxterm.rs` |
| `Services/SshConnectionFactory.cs`, `Connections/SshConnection.cs` | `core/ssh/*` |
| `Services/ErrorTranslator.cs` | `core/error.rs` |
| `Services/ClipboardService.cs` | A: plugin clipboard + retry; B: `arboard` + retry |
| `Services/ThemeManager.cs`, `LocalizationManager.cs` | A: `theme.css`, `i18n/*.json`; B: `theme.rs`, `i18n.rs` (cùng file JSON) |
| `ViewModels/MainViewModel.cs`, `SessionListViewModel.cs` | A: `stores/*.ts`; B: `app.rs` |
| `ViewModels/TerminalTabViewModel.cs`, `Views/TerminalView.xaml.cs`, `wwwroot/terminal.js` | A: `components/terminal/*`; B: `widgets/terminal.rs` + `core/ssh/shell.rs` |
| `ViewModels/SftpViewModel.cs`, `TransferQueueViewModel.cs`, `Views/SftpPanel.*` | A: `components/sftp/*`; B: `widgets/sftp_panel.rs`; chung `core/ssh/sftp.rs` |
| `Views/*Dialog.xaml` | A: `components/dialogs/*`; B: `widgets/dialogs.rs` |

---

## 5. Các biện pháp "nhẹ hơn" cụ thể

1. **Không .NET, không JIT.** Profile release: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
2. **Một phiên SSH cho cả shell và SFTP** (nhiều kênh). Kênh SFTP mở **lười** khi người dùng chuyển sang tab SFTP lần đầu.
3. **Không timer vòng lặp**: keepalive do `russh` lo; lệnh thu thập thông tin máy chủ chạy một lần sau khi kết nối và khi bấm làm mới (giữ hành vi v1, xem `TerminalTabViewModel.cs` ~dòng 640).
4. **Gom output** mỗi ~16 ms hoặc 64 KB trước khi đưa lên UI (như v1 mục 8.3).
5. **Lưu ngay nhưng gộp**: ghi `sessions.json` qua debounce 200 ms, vẫn ghi `.tmp` rồi đổi tên.
6. **A:** một WebView2 cho cả app; terminal không chọn `display:none`, chọn lại `fit()`+`focus()`; đóng tab `dispose()`; truyền output qua **Channel nhị phân** (`tauri::ipc::Channel` + `InvokeResponseBody::Raw`, fallback Base64 nếu phiên bản chưa hỗ trợ); xterm.js **WebGL** renderer; Svelte không framework UI nặng (bundle < 150 KB chưa tính xterm); danh sách SFTP ảo hóa; tài nguyên nhúng vào exe, **không còn thư mục `wwwroot`**.
7. **B:** vẽ lại chỉ khi có sự kiện (egui `request_repaint` theo dữ liệu đến, không vòng lặp 60 fps khi yên); cache glyph; grid terminal chỉ vẽ phần nhìn thấy; scrollback giới hạn theo `Scrollback`.

---

## 6. Giao diện — giữ nguyên bố cục v1

Bố cục cửa sổ chính **y hệt mục 6.1 v1** (toolbar; cột trái 2 tab Sessions/SFTP; splitter; tab ngang + terminal; status bar). Kích thước mặc định 1100×700, cột trái 400 px (min 140, max 600), font `Segoe UI` 13 px, tiêu đề `web-01 — SN Term`… đều giữ. Chừa sẵn chỗ cho Phần 2: một nút trên toolbar và một ô chữ ở góc phải status bar (ẩn khi chưa có tính năng).

- Màu: biến `--theme-*` (A) / `Theme` struct (B) lấy đúng mã màu từ `ThemeManager.cs` cho Dark và Light.
- Splitter 5 px, lưu `LeftColumnWidth`. Nhóm thu gọn lưu `CollapsedGroups`. Chọn nhiều Ctrl/Shift/Ctrl+A. Menu chuột phải tự vẽ. Phím tắt Enter/F2/Delete/Ctrl+D.
- Hộp thoại là modal trong cùng cửa sổ, có **hàng đợi hộp thoại** như v1 mục 6.6.
- Kéo thả từ Explorer: A dùng sự kiện `tauri://drag-drop`; B dùng winit `DroppedFile`. Thả vào SFTP → upload; thả `.snterm` vào cửa sổ → Import.
- A: tắt menu chuột phải mặc định, phím tắt trình duyệt (F5, Ctrl+P, Ctrl+F, zoom) như v1.

---

## 7. Dữ liệu và tương thích v1

| File | v2 đọc | v2 ghi | Ghi chú |
|---|---|---|---|
| `%APPDATA%\SNTerm\sessions.json` | ✔ nguyên định dạng v1 | ✔ `version: 1`, tên trường PascalCase y hệt | v1 bỏ qua trường lạ → quay lại v1 vẫn chạy; DPAPI cùng entropy |
| `settings.json`, `known_hosts.json` | ✔ | ✔ | Fingerprint SHA256 Base64 như v1 |
| `backups\`, `keys\` | ✔ | ✔ cùng quy tắc | |
| `%LOCALAPPDATA%\SNTerm\logs\` | — | ✔ `snterm-v2-YYYYMMDD.log` | |
| `%LOCALAPPDATA%\SNTerm\WebView2\` | — | A ✔ (`data_directory`); B không dùng | |
| `.snterm` | ✔ v1 | ✔ v1 byte-compatible | Test chéo với fixture v1 thật |
| `.mxtsessions` | ✔ | — | Chuyển `MobaXtermImporter.cs` sang Rust, dùng lại test mẫu |

GUID ghi dạng `xxxxxxxx-xxxx-…` chữ thường (AAD của `.snterm` là chuỗi này). Tuyệt đối không "sửa đẹp" tên trường JSON.

---

## 8. SSH, terminal, SFTP

### 8.1 Phiên kết nối (`core/ssh`, mỗi tab một `SshSession`)
- `russh::client::connect`, `Config { keepalive_interval = KeepAliveSeconds, … }`, timeout 15 s.
- Xác thực theo v1 mục 7.2: key → password → keyboard-interactive (trả lời mọi prompt bằng password).
- Host key: `check_server_key` → fingerprint SHA256 → `known_hosts.json` → `Trusted` / `NewHost` / `Changed` → gửi `CoreEvent::HostKeyPrompt` lên UI, chờ trả lời (oneshot, timeout 5 phút).
- Kênh shell: `request_pty("xterm-256color", cols, rows)` + `request_shell`; kênh SFTP: `request_subsystem("sftp")` + `russh_sftp`; kênh exec cho lệnh `MON|…` (timeout 2 s, không in ra terminal).
- Mất kết nối → `Disconnected`, in `[Đã ngắt kết nối] Nhấn R hoặc Enter để kết nối lại`; R/Enter → kết nối lại; mật khẩu giữ trong RAM của tab, zeroize khi đóng.

### 8.2 Giao thức UI ⇄ core (`CoreCommand` / `CoreEvent`)
Lệnh: `TerminalOpen{session_id, cols, rows} -> tab_id`, `TerminalInput{tab_id, bytes}`, `TerminalResize`, `TerminalReconnect`, `TerminalClose`, `Sftp*` (mục 8.4), `ClipboardCopy/Read`.
Sự kiện: `TerminalOutput{tab_id, bytes}` (đã gom), `TabStatus{tab_id, status, message}`, `TabMonitor{tab_id, os, uptime, mem, disks, …}`, `DialogRequest{kind: HostKey|Password, tab_id, …}`, `TransferProgress{id, name, bytes, total, speed}`.
A ánh xạ sang `invoke`/`Channel`/`emit`; B gọi trực tiếp qua `mpsc`.

### 8.3 Terminal
- **A:** `TerminalPane.ts` giữ nguyên logic `terminal.js` v1: chờ font nạp mới `open()`, `ResizeObserver` + `fit()` debounce 100 ms, copy-on-select ở `mouseup`, chuột phải theo `RightClickAction` ⊕ Shift, `attachCustomKeyEventHandler` cho phím tắt v1 mục 8.6, `term.paste()`.
- **B:** `widgets/terminal.rs`: `alacritty_terminal::Term` + `vte` parser nhận `TerminalOutput`; vẽ grid (nền ô, chữ theo run cùng màu/kiểu, con trỏ nhấp nháy theo `CursorBlink`, vùng chọn); chuột: kéo chọn (ký tự/từ/dòng theo số lần nhấp), lăn cuộn scrollback, chuột phải theo cài đặt; bàn phím: bảng ánh xạ phím → chuỗi escape (mũi tên, F1–F12, Home/End, PgUp/PgDn, Ctrl+chữ, Alt+chữ, chế độ application cursor), `Event::Ime` cho bộ gõ, phím tắt ứng dụng xử lý trước khi tới terminal (không chiếm `Ctrl+C`, `Ctrl+V`, `Ctrl+W` thường); đổi kích thước → `Term::resize` + `TerminalResize`.

### 8.4 SFTP (`core/ssh/sftp.rs`)
Lệnh `SftpList`, `SftpMkdir`, `SftpRename`, `SftpRemove(paths, recursive)`, `SftpChmod(paths, mode, recursive)`, `SftpUpload(local_paths, remote_dir)`, `SftpDownload(remote_paths, local_dir)`, `SftpOpenInEditor(remote_path)`, `SftpCancel(transfer_id)`. Hàng đợi chạy lần lượt, hủy thì xóa file dở. Mở bằng trình soạn thảo: tải về `%LOCALAPPDATA%\SNTerm\edit\<tab>\…`, mở bằng app mặc định hoặc `CustomEditorPath`, `notify` theo dõi → tự upload lại. Lỗi quyền → "Không có quyền thực hiện thao tác này".

---

## 9. Export / Import
Giữ nguyên v1 mục 5.4, 6.7, 6.8. Thêm kiểm thử chéo: `tests/fixtures/` có **file `.snterm` do v1 tạo thật** (2 chế độ) và test Rust phải import được; file v2 tạo phải mở được bằng v1 (**[Tay]** một lần ở Giai đoạn 5).

---

## 10. Chừa sẵn cho Phần 2 (Đồng bộ VM)
Phần này **không** làm tính năng đồng bộ, chỉ đảm bảo Phần 2 cắm vào được mà không phải sửa lại:
- `Session` có trường `UpdatedAt` (serde default = `CreatedAt` khi thiếu) và envelope `sessions.json` chấp nhận trường `Deleted` (giữ nguyên khi đọc/ghi lại, không được làm mất).
- `AppSettings` giữ nguyên khối `Sync` lạ nếu có (serde `flatten` map `extra`), không làm mất khi ghi.
- Toolbar có vị trí nút thứ 4 (giữa Import và Cài đặt) và status bar có ô bên phải, cả hai ẩn qua cờ `features.sync`.
- `core/crypto/snterm_file.rs` tách riêng hàm `encrypt_vault(sessions, password, extra_json)` / `decrypt_vault` để Phần 2 dùng lại nguyên vẹn.
- Nếu Phần 2 đã **phát hành trên v1 (C#) trước** khi v2 xong: Giai đoạn 7 dưới đây chuyển module Sync sang Rust theo đúng định dạng vault của Phần 2; nếu chưa, Giai đoạn 7 bỏ qua và Phần 2 làm thẳng trên v2.

---

## 11. Các giai đoạn triển khai

### Giai đoạn 0 — Spike và chọn phương án
- Dựng workspace `v2/`, crate `core` rỗng, cả `app-tauri` và `app-native` ở mức "cửa sổ trống đúng bố cục v1 với dữ liệu giả" (toolbar, 2 cột, splitter, tab bar, status bar, theme sáng/tối, vi/en).
- **Spike SSH (1–2 ngày):** `core/ssh` kết nối VM thật bằng password, key OpenSSH, key `.ppk`, keyboard-interactive; mở shell, `window_change`, chạy `htop`; mở SFTP trên cùng phiên, `ls`. Thất bại → đổi lõi sang Go + Wails (giữ thiết kế).
- **Spike B (chỉ nếu đang cân nhắc B, 2–3 ngày):** widget terminal tối thiểu bằng `alacritty_terminal` + egui: hiển thị `htop`, `vim`, gõ tiếng Việt bằng Unikey/EVKey trong `nano`, chạy qua Remote Desktop (không GPU).
- Chốt A hoặc B với người dùng (bảng mục 1 + kết quả spike).

**Hoàn thành khi:** build ra exe của phương án đã chọn ≤ 12 MB; **[Tay]** bố cục giống v1 khi đặt cạnh nhau (chụp 2 ảnh), kéo splitter, đổi theme/ngôn ngữ; spike SSH qua.

### Giai đoạn 1 — Dữ liệu tương thích v1 + quản lý VM
`core/store`, `core/crypto/dpapi.rs` (cùng entropy), sao lưu hằng ngày, khôi phục file hỏng; danh sách VM đầy đủ; form Thêm/Sửa; nhân bản; nhóm. Test: đọc `sessions.json` **do v1 tạo thật**, ghi ra rồi v1 đọc lại được, DPAPI round-trip.

**Hoàn thành khi:** test qua; **[Tay]** chạy v2 trên máy đang có v1 → đủ VM, đúng nhóm; thêm/sửa/xóa rồi mở lại **v1** vẫn đúng.

### Giai đoạn 2 — SSH và terminal
`core/ssh` đầy đủ, HostKey/Password dialog, hàng đợi hộp thoại, key OpenSSH/PKCS#8/PPK, mật khẩu đã lưu sai → hỏi lại và cập nhật, terminal (A: xterm.js nhiều instance + Channel; B: widget), thanh tab đầy đủ (v1 mục 6.6), mở nhiều VM song song, "Kiểm tra kết nối", thông tin máy chủ → icon OS + tooltip, R/Enter kết nối lại.

**Hoàn thành khi:** toàn bộ **[Tay]** Giai đoạn 2 v1 qua; thêm: mở 10 tab → RAM tăng ≤ 80 MB (A) / ≤ 30 MB (B) so với 1 tab; `netstat` 1 kết nối/tab; Unikey/EVKey trong `nano` đúng dấu; `cat` file vài MB không treo.

### Giai đoạn 3 — Copy / Paste / phím tắt
Như Giai đoạn 3 v1 (clipboard retry 5 × 50 ms, xác nhận dán nhiều dòng, menu chuột phải, `Ctrl+C` dừng được `ping`).

### Giai đoạn 4 — SFTP
Toàn bộ mục 9 v1 + chmod, mở bằng trình soạn thảo rồi tự upload, tìm nhanh bằng bàn phím (chuyển `SftpTypeAheadTests.cs`), danh sách ảo hóa.

**Hoàn thành khi:** **[Tay]** Giai đoạn 4 v1 qua; thư mục 10.000 file cuộn mượt; chmod đệ quy đúng; sửa bằng Notepad rồi lưu → file trên VM đổi.

### Giai đoạn 5 — Export / Import
`snterm_file.rs`, `mobaxterm.rs`, hộp thoại Export/Import, kéo thả, mở `.snterm` bằng nhấp đúp (single-instance + tham số dòng lệnh). Toàn bộ test bắt buộc Giai đoạn 5 v1 chuyển sang Rust + test chéo với fixture v1.

**Hoàn thành khi:** test qua; **[Tay]** file v2 export mở được bằng v1 và ngược lại.

### Giai đoạn 6 — Cài đặt và hoàn thiện
Settings đầy đủ, áp dụng ngay cho mọi tab; nhớ cửa sổ; log; bắt panic/lỗi toàn cục (không tự tắt); hỏi khi đóng còn tab kết nối; A: kiểm tra WebView2 Runtime lúc khởi động (bootstrapper Tauri hoặc thông báo kèm link như v1).

### Giai đoạn 7 — Chuyển module Đồng bộ (chỉ khi Phần 2 đã có trên v1)
Chuyển `Services/Sync/*` C# sang `core/sync` theo tài liệu Phần 2, dùng lại test và fixture vault.

### Giai đoạn 8 — Đóng gói và tài liệu
`cargo tauri build` (A) hoặc `cargo build --release` (B) → exe portable + bộ cài (NSIS của Tauri, hoặc cập nhật `SNTerm.iss`; chọn một). Liên kết đuôi `.snterm`. README: yêu cầu mới, cách chuyển từ v1 (không cần làm gì), bảng hiệu năng. Đánh dấu `src/SNTerm/` là legacy.

### Giai đoạn 9 — Đo và chốt
Đo chỉ tiêu mục 1 trên cùng máy cho v1 và v2 (3 lần lấy trung bình), ghi README. Không đạt → tối ưu tiếp hoặc ghi rõ lý do.

---

## 12. Rủi ro

| Rủi ro | Mức | Xử lý |
|---|---|---|
| `russh` thiếu/khác API ở phiên bản dùng | Trung bình | Spike Giai đoạn 0 với VM thật; thất bại → Go + Wails, giữ thiết kế |
| Key `.ppk` không có crate sẵn | Trung bình | `crypto/ppk.rs` cho PPK v2/v3 theo tài liệu PuTTY, test key mẫu; gợi ý `puttygen` chuyển sang OpenSSH khi đọc lỗi |
| **B:** bộ gõ tiếng Việt với winit | Trung bình | Spike sớm với Unikey và EVKey (cả chế độ Unicode + backspace); nếu lỗi, xử lý `WM_CHAR`/`Ime::Commit` thủ công |
| **B:** không có GPU (Remote Desktop, máy ảo) | Trung bình | Backend `glow` + fallback renderer phần mềm; kiểm tra qua RDP ở spike |
| **B:** chi tiết terminal (chữ đôi rộng, emoji, URL, cuộn) | Trung bình | Dùng `alacritty_terminal` cho logic, chỉ tự vẽ; đối chiếu với Alacritty thật trên cùng output |
| **A:** Tauri Channel chưa hỗ trợ payload thô | Thấp | Fallback Base64 như v1 |
| Tiếng Việt IME trong WebView2 (A) | Thấp | Cùng engine v1; kiểm tra **[Tay]** sớm |
| Chạy song song v1 và v2 trên cùng máy | Thấp | Cả hai ghi nguyên tử, v2 đọc lại trước khi ghi; README khuyên dùng một bản |
| SmartScreen với exe chưa ký | Thấp | Như v1; ký số tùy chọn |

---

## 13. Kiểm thử nhanh khi chưa có VM
Như v1 mục 12 (WSL + `openssh-server`). v2 hỗ trợ biến môi trường `SNTERM_DATA_DIR=<thư mục khác>` (chỉ để kiểm thử) để chạy với dữ liệu riêng không đụng v1.
