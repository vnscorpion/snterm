# SN Term v2 — Kế hoạch viết lại bằng Rust + Tauri 2

> Viết lại **SN Term** bằng ngôn ngữ khác để **chạy nhẹ hơn** (ít RAM, khởi động nhanh, không cần cài .NET), **giữ nguyên bố cục và toàn bộ tính năng** của bản v1 (C# / WPF), và **thêm tính năng Đồng bộ danh sách VM** giữa nhiều máy.
>
> Tài liệu này là bản kế hoạch tiếp nối `SN Term.md` (v1). Những gì v1 đã mô tả (bố cục, quy tắc form, định dạng file, thông báo tiếng Việt…) **vẫn là chuẩn**; tài liệu này chỉ ghi phần **khác đi** và phần **mới**.

---

## 0. Hướng dẫn cho AI/CLI thực hiện kế hoạch này

1. Đọc `SN Term.md` (v1) trước, rồi đọc file này. Mục nào file này không nhắc tới thì làm **y như v1**.
2. Mã nguồn v1 trong `src/SNTerm/` là **tài liệu tham chiếu hành vi**: khi phân vân, mở file C# tương ứng (bảng ánh xạ ở mục 4.3) và làm cho giống.
3. Làm **tuần tự từng giai đoạn** (mục 11). Sau mỗi giai đoạn: `cargo build --release` không lỗi, `cargo test` và `npm test` qua, rồi `git commit` với thông điệp `v2 Giai đoạn N: <tóm tắt>`.
4. **Không đổi bố cục** cửa sổ chính: vẫn thanh công cụ trên cùng, cột trái 2 tab Sessions/SFTP, thanh chia kéo được, cột phải thanh tab ngang + terminal, thanh trạng thái dưới cùng. Màu sắc lấy đúng mã màu trong `ThemeManager.cs`.
5. **Tương thích dữ liệu**: v2 đọc/ghi **cùng thư mục** `%APPDATA%\SNTerm\` và **cùng định dạng** `sessions.json`, `settings.json`, `known_hosts.json`, file `.snterm` của v1. Người dùng cài v2 đè lên v1 phải thấy đủ VM, nhấp đúp vào thẳng, không phải nhập lại gì. Quay lại v1 vẫn chạy được.
6. Mọi chữ hiển thị bằng **tiếng Việt có dấu** (có bản tiếng Anh như v1). Tên biến, hàm, file bằng tiếng Anh.
7. Không ghi mật khẩu, passphrase, mật khẩu đồng bộ ra log hay file dạng chữ thường.
8. Mọi thao tác mạng chạy bất đồng bộ trong Rust (tokio); không bao giờ chặn luồng giao diện.
9. Người dùng không phải lập trình viên: báo cáo ngắn gọn, nói rõ cần họ làm gì, liệt kê các mục **[Tay]** cần tự bấm thử.

---

## 1. Mục tiêu và chỉ tiêu "nhẹ hơn"

| Chỉ tiêu | v1 (C# / WPF) | **v2 mục tiêu** | Cách đo |
|---|---|---|---|
| Yêu cầu cài trước | .NET 10 Desktop Runtime (~60 MB) + WebView2 Runtime | **Chỉ WebView2 Runtime** (có sẵn trên Windows 10/11) | — |
| Dung lượng file chạy | ~7 MB exe + thư mục `wwwroot` | **1 file `SNTerm.exe` ≤ 10 MB**, không thư mục kèm | Kích thước file |
| RAM lúc mở app, 0 tab | ~120–150 MB | **≤ 90 MB** | Task Manager, cột "Memory" tổng các tiến trình SNTerm + WebView2 của nó |
| RAM thêm mỗi tab terminal | ~50–80 MB (một WebView2 riêng mỗi tab) | **≤ 8 MB** (chỉ thêm một `Terminal` xterm.js trong cùng trang) | Mở 10 tab, so với 1 tab |
| Thời gian khởi động tới khi thấy danh sách VM | ~1.5–3 s (JIT .NET + WPF) | **≤ 0.7 s** | Bấm giờ từ nhấp đúp exe |
| Kết nối TCP mỗi tab | 2 (SSH + SFTP riêng, hạn chế của SSH.NET) | **1** (SFTP mở trên kênh phụ của cùng phiên SSH) | `netstat` |
| CPU khi để yên | ~0–1 % | **0 %** (không timer vòng lặp, keepalive do thư viện SSH lo) | Task Manager |

Các số v1 là ước lượng; **Giai đoạn 9** sẽ đo lại cả hai bản trên cùng máy và ghi vào README.

**Giữ nguyên:** toàn bộ tính năng và bố cục v1 (mục 2). **Thêm mới:** Đồng bộ danh sách VM (mục 10).

---

## 2. Phạm vi

**Giữ nguyên từ v1 (phải có đủ):**
- Quản lý VM: thêm/sửa/nhân bản/xóa, nhóm, đổi tên nhóm, chuyển nhóm, tìm kiếm, nhóm "Gần đây", chọn nhiều, phím tắt, mật khẩu mã hóa DPAPI, key file + passphrase, ⚠ thiếu key.
- Kết nối SSH: vào thẳng khi đã lưu mật khẩu, hỏi mật khẩu khi cần, xác nhận host key, keyboard-interactive, mở nhiều VM song song (tối đa `MaxParallelConnects`), hàng đợi hộp thoại, phát hiện mất kết nối và **phím R / Enter để kết nối lại**, thông tin máy chủ (OS, uptime, RAM, đĩa, mạng) để hiện **icon hệ điều hành** trên danh sách và tooltip.
- Terminal xterm.js: bôi đen là copy, chuột phải dán/menu, xác nhận dán nhiều dòng, tiếng Việt (Unikey/EVKey), phím tắt `Ctrl+Shift+C/V/W`, `Ctrl+Tab`, `Alt+1..9`, `Ctrl+=/-/0`, font JetBrains Mono đóng gói sẵn, theme sáng/tối.
- Thanh tab: chấm trạng thái, đánh số tab trùng tên, cuộn ngang, kéo thả sắp xếp, chuột giữa đóng, menu chuột phải (Kết nối lại, Nhân bản, Đổi tên, Đóng…), giữ nguyên nội dung khi chuyển tab.
- SFTP: tự chuyển sang SFTP khi đăng nhập, đổi theo tab, duyệt, lên/làm mới/home, gõ đường dẫn, tạo thư mục, đổi tên, xóa, **chmod (có đệ quy)**, **mở file bằng trình soạn thảo** (mặc định hoặc đường dẫn tùy chỉnh rồi tự upload lại khi lưu), upload/download file và thư mục có tiến trình, kéo thả từ Explorer, hiện file ẩn, **tìm nhanh bằng bàn phím** (gõ chữ đầu).
- Export/Import `.snterm` (AES-256-GCM, PBKDF2 600.000 vòng, AAD = id), xử lý trùng (Bỏ qua/Ghi đè/Thêm bản sao), **Import từ MobaXterm `.mxtsessions`**, kéo thả file vào cửa sổ, mở file `.snterm` bằng nhấp đúp.
- Cài đặt: font, cỡ chữ, ngôn ngữ (vi/en), theme, copy-on-select, hành vi chuột phải, scrollback, keepalive, file ẩn, trình soạn thảo, nhớ kích thước cửa sổ và độ rộng cột trái.
- Sao lưu tự động `sessions.json`, khôi phục khi file hỏng, log sự cố, bắt lỗi toàn cục, hỏi xác nhận khi đóng còn tab đang kết nối.

**Thêm mới:** Đồng bộ danh sách VM giữa nhiều máy (mục 10).

**Vẫn ngoài phạm vi:** RDP, VNC, Telnet, Serial, X11, SSH tunnel, macro, multi-exec, kéo file từ SFTP ra Explorer, máy chủ đồng bộ riêng của SN Term (đồng bộ chỉ dùng kho của người dùng).

---

## 3. Công nghệ

### 3.1 Lựa chọn: **Rust + Tauri 2**, giao diện web (Svelte 5 + TypeScript), terminal xterm.js

| Thành phần | v1 | **v2** | Ghi chú |
|---|---|---|---|
| Ngôn ngữ lõi | C# / .NET 10 | **Rust** (stable, edition 2021) | Biên dịch thành mã máy, không runtime, an toàn bộ nhớ (quan trọng với mã hóa/mật khẩu) |
| Khung ứng dụng | WPF | **Tauri 2.x** | Một cửa sổ, **một WebView2 duy nhất** cho toàn bộ giao diện |
| Giao diện | XAML | **HTML/CSS + Svelte 5 + TypeScript**, build bằng Vite | Svelte biên dịch ra JS thuần rất nhỏ, không có runtime nặng |
| SSH | SSH.NET | **`russh`** | Thuần Rust, async (tokio), password / publickey / keyboard-interactive, nhiều kênh trên một phiên |
| SFTP | SSH.NET | **`russh-sftp`** | Chạy trên kênh phụ của cùng phiên SSH (1 kết nối TCP/tab) |
| Đọc key | SSH.NET | **`ssh-key`** (OpenSSH, PKCS#8, PEM, có passphrase) + **bộ đọc PPK tự viết** (v2/v3, v3 dùng crate `argon2`) | Xem rủi ro mục 12 |
| Terminal | xterm.js 6 trong WebView2 riêng mỗi tab | **xterm.js 6** + `addon-fit`, `addon-unicode11`, **`addon-webgl`** (fallback canvas) | Nhiều `Terminal` trong cùng một trang, ẩn/hiện bằng CSS |
| Mã hóa mật khẩu cục bộ | DPAPI | **DPAPI** qua crate `windows` (`CryptProtectData`), **giữ nguyên entropy** `SNTerm.DPAPI.Entropy.v1` | Để đọc được `sessions.json` của v1 |
| File `.snterm` | AES-256-GCM + PBKDF2 | `aes-gcm`, `pbkdf2`, `sha2`, `zeroize` | Byte-compatible với v1 |
| JSON | System.Text.Json | `serde` + `serde_json` | `#[serde(default)]`, bỏ qua trường lạ |
| Clipboard, hộp thoại file, mở app ngoài, log, trạng thái cửa sổ, một instance | WPF | plugin Tauri: `clipboard-manager`, `dialog`, `opener`, `log`, `window-state`, `single-instance` | |
| Test | xUnit | `cargo test` (Rust) + `vitest` (TypeScript) | |
| Đóng gói | `dotnet publish` + Inno Setup | `cargo tauri build` → exe portable + bộ cài NSIS; cập nhật `SNTerm.iss` nếu vẫn dùng Inno | |

**Vì sao không chọn cách khác:**
- *Giữ C# nhưng tối ưu*: vẫn cần .NET Runtime, WPF khởi động chậm; không đạt chỉ tiêu mục 1.
- *Go + Wails*: thư viện SSH của Go rất chín, nhưng Wails v3 chưa ổn định bằng Tauri 2, exe to hơn (~15 MB) và RAM nhỉnh hơn. Đây là **phương án dự phòng** nếu `russh` gặp vấn đề không giải quyết được (mục 12): kiến trúc, giao diện web, giao thức tin nhắn và tính năng đồng bộ trong tài liệu này dùng lại được nguyên vẹn, chỉ đổi phần lõi.
- *Electron*: nặng hơn v1, loại.
- *Rust giao diện native (egui/iced) + tự viết terminal*: nhẹ nhất lúc chạy nhưng bộ gõ tiếng Việt (IME) trong egui chưa tốt, và phải tự làm trình giả lập terminal. Không đáng rủi ro.

### 3.2 Tài nguyên đóng gói trong exe
- Tauri nhúng toàn bộ thư mục `dist/` (HTML, JS, CSS, font `.woff2`, icon OS) vào exe → **không còn thư mục `wwwroot`** bên cạnh.
- Phiên bản xterm.js và addon ghi trong `package.json`; không dùng CDN.

---

## 4. Kiến trúc và cấu trúc thư mục

### 4.1 Mô hình

```
┌──────────────────────── SNTerm.exe (Rust, tokio) ────────────────────────┐
│  commands/*  ← tauri::command (invoke từ JS)                               │
│  state: AppState { sessions, settings, known_hosts, tabs: HashMap<TabId,…>}│
│  ssh/   russh Handle + kênh shell + kênh sftp (mỗi tab 1 phiên)            │
│  sync/  SyncEngine (merge, vault, backend Folder | Sftp)                   │
│  store/ sessions.json, settings.json, known_hosts.json, backups, keys      │
│  crypto/ dpapi, snterm_file (AES-GCM/PBKDF2), ppk                          │
└───────────────┬──────────────────────────────────────────────┬────────────┘
                │ invoke (lệnh)        Channel nhị phân (output terminal)  │ emit (sự kiện)
┌───────────────┴──────────────────────────────────────────────┴────────────┐
│  WebView2 duy nhất — Svelte                                               │
│  Toolbar │ LeftPanel(Sessions | Sftp) │ Splitter │ TabBar + TerminalHost  │
│  TerminalHost: <div data-tab=…> × N, mỗi div 1 xterm.js, ẩn/hiện bằng CSS │
│  StatusBar (trạng thái kết nối • trạng thái đồng bộ)                       │
└───────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Thư mục

```
SNTerm/
├─ SN Term.md                 // kế hoạch v1 (giữ)
├─ SN Term v2.md              // file này
├─ src/SNTerm/                // mã v1 C#, giữ nguyên để tham chiếu tới khi v2 phát hành
├─ v2/
│  ├─ package.json, vite.config.ts, tsconfig.json
│  ├─ src/                    // giao diện Svelte + TS
│  │  ├─ App.svelte                 // bố cục 2 cột (mục 6)
│  │  ├─ lib/ipc.ts                 // bọc invoke/listen/Channel, kiểu dữ liệu dùng chung
│  │  ├─ lib/i18n/{vi,en}.json      // chuyển từ LocalizationManager.cs
│  │  ├─ lib/theme.css              // biến màu từ ThemeManager.cs
│  │  ├─ components/
│  │  │  ├─ Toolbar.svelte, StatusBar.svelte
│  │  │  ├─ sessions/ SessionList.svelte, SessionItem.svelte, GroupHeader.svelte
│  │  │  ├─ terminal/ TabBar.svelte, TabItem.svelte, TerminalHost.svelte, TerminalPane.ts (xterm)
│  │  │  ├─ sftp/ SftpPanel.svelte, SftpList.svelte, TransferBar.svelte
│  │  │  └─ dialogs/ SessionEditor, PasswordPrompt, HostKey, Settings, Export, Import, Chmod, Input, SyncSetup, SyncPreview
│  │  └─ stores/ sessions.ts, tabs.ts, settings.ts, sync.ts
│  ├─ public/fonts/JetBrainsMono-*.woff2, OFL.txt ; public/os/*.png
│  └─ src-tauri/
│     ├─ Cargo.toml, tauri.conf.json, build.rs, icons/
│     └─ src/
│        ├─ main.rs, lib.rs, state.rs, error.rs (ErrorTranslator → tiếng Việt)
│        ├─ commands/ sessions.rs, terminal.rs, sftp.rs, export_import.rs, settings.rs, sync.rs, clipboard.rs
│        ├─ ssh/ connection.rs, auth.rs, host_key.rs, shell.rs, sftp.rs, monitor.rs
│        ├─ store/ paths.rs, session_store.rs, settings_store.rs, known_hosts.rs, backup.rs
│        ├─ crypto/ dpapi.rs, snterm_file.rs, ppk.rs
│        ├─ import/ mobaxterm.rs
│        └─ sync/ engine.rs, vault.rs, merge.rs, backend/{mod.rs, folder.rs, sftp.rs}
└─ tests/ (v1 giữ) ; v2 test nằm trong từng crate/module và v2/src/**/*.test.ts
```

### 4.3 Bảng ánh xạ v1 → v2 (để biết "làm giống cái gì")

| v1 (C#) | v2 |
|---|---|
| `Models/SessionInfo.cs` | `store/session_store.rs` (`Session`), kiểu TS trong `lib/ipc.ts` |
| `Services/SessionStore.cs`, `SettingsStore.cs`, `KnownHostsStore.cs`, `AppPaths.cs` | `store/*` |
| `Services/SecretProtector.cs` | `crypto/dpapi.rs` |
| `Services/SessionExporter.cs`, `SessionImporter.cs`, `Models/ExportFile.cs` | `crypto/snterm_file.rs`, `commands/export_import.rs` |
| `Services/MobaXtermImporter.cs` | `import/mobaxterm.rs` |
| `Services/SshConnectionFactory.cs`, `Connections/SshConnection.cs` | `ssh/*` |
| `Services/ErrorTranslator.cs` | `error.rs` |
| `Services/ClipboardService.cs` | plugin clipboard + retry trong `commands/clipboard.rs` |
| `Services/ThemeManager.cs`, `LocalizationManager.cs` | `lib/theme.css`, `lib/i18n/*.json` |
| `ViewModels/MainViewModel.cs`, `SessionListViewModel.cs` | `stores/tabs.ts`, `stores/sessions.ts` |
| `ViewModels/TerminalTabViewModel.cs`, `Views/TerminalView.xaml.cs`, `wwwroot/terminal.js` | `components/terminal/*`, `ssh/shell.rs`, `ssh/monitor.rs` |
| `ViewModels/SftpViewModel.cs`, `TransferQueueViewModel.cs`, `Views/SftpPanel.*` | `components/sftp/*`, `ssh/sftp.rs` |
| `Views/*Dialog.xaml` | `components/dialogs/*` |

---

## 5. Các biện pháp "nhẹ hơn" cụ thể

1. **Một WebView2 cho cả ứng dụng.** v1 tạo một WebView2 cho mỗi tab (mỗi cái vài chục MB và một luồng render). v2 đặt tất cả `Terminal` xterm.js trong cùng một trang; tab không chọn thì `display:none`, khi chọn lại thì `fit()` + `focus()`. Đóng tab → `term.dispose()`.
2. **Không .NET, không JIT.** Exe Rust khởi động ngay; profile release: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
3. **Một phiên SSH cho cả shell và SFTP** (russh cho phép nhiều kênh). Giảm một nửa kết nối, một nửa thời gian đăng nhập, không hỏi mật khẩu hai lần.
4. **Truyền output terminal bằng Channel nhị phân** của Tauri 2 (`tauri::ipc::Channel` với `InvokeResponseBody::Raw`), không qua JSON/Base64. Vẫn gom dữ liệu mỗi ~16 ms hoặc 64 KB như v1 (mục 8.3 v1). Nếu phiên bản Tauri đang dùng chưa hỗ trợ payload thô → tạm dùng Base64 như v1, ghi chú lại.
5. **xterm.js WebGL renderer**: giảm CPU khi `cat` file lớn, `htop`; tự fallback canvas nếu WebGL lỗi (`onContextLoss`).
6. **Không timer vòng lặp**: keepalive do `russh` cấu hình (`keepalive_interval`); lệnh thu thập thông tin máy chủ (OS, RAM, đĩa) chạy **một lần sau khi kết nối** và khi người dùng bấm làm mới, không lặp định kỳ (v1 cũng chỉ chạy khi cần; giữ nguyên hành vi, kiểm tra lại trong `TerminalTabViewModel.cs` dòng ~640).
7. **Giao diện không framework nặng**: Svelte biên dịch ra DOM thuần; không Material/Fluent UI; CSS tự viết theo mã màu v1. Bundle JS (không tính xterm) mục tiêu < 150 KB.
8. **Danh sách SFTP ảo hóa** (chỉ render hàng đang thấy) để thư mục 10.000 file vẫn mượt.
9. **Lưu ngay nhưng gộp**: ghi `sessions.json` qua hàng đợi debounce 200 ms (vẫn ghi `.tmp` rồi đổi tên), tránh ghi đĩa dồn dập khi nhập liệu nhanh.

---

## 6. Giao diện — giữ nguyên bố cục v1

Bố cục cửa sổ chính **y hệt mục 6.1 của v1** (toolbar, cột trái 2 tab, splitter, tab ngang + terminal, status bar). Sơ đồ, kích thước mặc định (cửa sổ 1100×700, cột trái 400 px, min 140, max 600), font `Segoe UI` 13 px, tiêu đề cửa sổ `web-01 — SN Term`… đều giữ.

Khác biệt duy nhất (do tính năng mới, giữ tối thiểu):

```
│ [+ Thêm VM]  [⇪ Export]  [⇩ Import]  [⟳ Đồng bộ]  [⚙ Cài đặt]           │   ← thêm 1 nút
…
│ Đã kết nối web-01 (10.0.0.5:22)                 Đồng bộ: 14:32 ✓  (OneDrive) │   ← góc phải status bar
```

- Nút **⟳ Đồng bộ**: chưa bật đồng bộ → mở hộp thoại thiết lập (mục 10.7); đã bật → đồng bộ ngay, icon xoay khi đang chạy, đỏ khi lỗi (tooltip ghi lỗi).
- Status bar bên phải: `Đồng bộ: HH:mm ✓` / `Đang đồng bộ…` / `Lỗi đồng bộ: <lý do>` / ẩn nếu tắt. Nhấp vào → mở tab Đồng bộ trong Cài đặt.
- Hộp thoại Cài đặt thêm tab **"Đồng bộ"** (mục 10.7). Các tab khác giữ như v1.

Cách dựng giao diện web cho giống WPF:
- `lib/theme.css` khai báo biến `--theme-window-bg`, `--theme-toolbar-bg`, … lấy đúng mã màu từ `ThemeManager.cs` cho cả Dark và Light.
- Splitter: thanh 5 px, kéo bằng pointer events, lưu `LeftColumnWidth` vào `settings.json` như v1.
- Danh sách VM: nhóm có thể thu gọn (lưu `CollapsedGroups`), chọn nhiều bằng Ctrl/Shift/Ctrl+A, menu chuột phải tự vẽ (HTML), phím tắt Enter/F2/Delete/Ctrl+D.
- Hộp thoại: modal HTML trong cùng trang (nhanh hơn mở cửa sổ con), có hàng đợi hộp thoại như v1 mục 6.6.
- Kéo thả từ Explorer: sự kiện `tauri://drag-drop` của cửa sổ (Tauri 2 cấp đường dẫn file thật) → upload SFTP hoặc Import `.snterm` tùy thả vào đâu.
- Menu chuột phải mặc định của WebView2, phím tắt trình duyệt (F5, Ctrl+P, Ctrl+F, zoom) phải **tắt** như v1.

---

## 7. Dữ liệu và tương thích với v1

| File | v2 đọc | v2 ghi | Ghi chú |
|---|---|---|---|
| `%APPDATA%\SNTerm\sessions.json` | ✔ nguyên định dạng v1 (`{version, sessions[]}`) | ✔ `version: 1`, **thêm trường** `updatedAt`, `deleted`, `deletedAt` cho đồng bộ (mục 10.3) | v1 bỏ qua trường lạ → quay lại v1 vẫn chạy. DPAPI cùng entropy nên mật khẩu giải mã được |
| `settings.json` | ✔ | ✔ thêm khối `sync` (mục 10.7) | |
| `known_hosts.json` | ✔ | ✔ | Fingerprint SHA256 Base64 giống OpenSSH/SSH.NET |
| `backups\` | ✔ | ✔ cùng quy tắc (hằng ngày, trước Import, **trước lần đồng bộ đầu**) | |
| `keys\` | ✔ | ✔ `<id>_<tên key>` | |
| `%LOCALAPPDATA%\SNTerm\logs\` | — | ✔ `snterm-v2-YYYYMMDD.log` | |
| `%LOCALAPPDATA%\SNTerm\WebView2\` | — | ✔ Tauri đặt `data_directory` tại đây | |
| `.snterm` | ✔ v1 | ✔ v1 (byte-compatible) | Test chéo bằng file do v1 tạo thật |
| `.mxtsessions` | ✔ | — | Chuyển `MobaXtermImporter.cs` sang Rust, dùng lại test mẫu |

Quy tắc serde: mọi struct `#[serde(default)]`, tên trường **PascalCase** đúng như v1 (`"Name"`, `"EncryptedPassword"`…) cho `sessions.json`/`settings.json`/`known_hosts.json`; **camelCase** cho `.snterm` (như v1 đã làm). Tuyệt đối không "sửa đẹp" tên trường.

GUID: v1 ghi `Id` dạng `xxxxxxxx-xxxx-…` chữ thường; v2 dùng crate `uuid`, giữ đúng dạng này (AAD của `.snterm` là chuỗi này).

---

## 8. SSH, terminal, SFTP bằng Rust

### 8.1 Phiên kết nối (mỗi tab một `SshSession`)
- `russh::client::connect` với `Config { keepalive_interval, inactivity_timeout: None, … }`, timeout kết nối 15 s.
- Thứ tự xác thực như v1 mục 7.2: key (nếu có) → password → keyboard-interactive (trả lời mọi prompt bằng password).
- Host key: `Handler::check_server_key` tính SHA256 fingerprint → so `known_hosts.json` → `Trusted` đi tiếp; `NewHost`/`Changed` → gửi sự kiện cho giao diện hiện `HostKeyDialog`, chờ trả lời (oneshot) với timeout 5 phút.
- Sau xác thực: mở **kênh shell** (`request_pty("xterm-256color", cols, rows)` + `request_shell`) và, khi cần, **kênh SFTP** (`request_subsystem("sftp")` + `russh_sftp::client::SftpSession`) trên cùng phiên. Kênh SFTP mở **lười** khi cột trái chuyển sang tab SFTP lần đầu.
- Lệnh thu thập thông tin máy chủ (chuỗi `MON|…` trong `TerminalTabViewModel.cs`) chạy trên **kênh exec riêng** sau khi shell sẵn sàng, timeout 2 s; không in ra terminal.
- Mất kết nối (kênh đóng, lỗi đọc, keepalive thất bại) → trạng thái `Disconnected`, in `[Đã ngắt kết nối] Nhấn R hoặc Enter để kết nối lại`; phím R/Enter → kết nối lại, giữ mật khẩu trong RAM của tab (zeroize khi đóng tab).

### 8.2 Giao thức giao diện ⇄ lõi (thay cho `postMessage` v1)

JS → Rust (`invoke`): `terminal_open(sessionId, cols, rows) -> tabId`, `terminal_input(tabId, data)`, `terminal_resize(tabId, cols, rows)`, `terminal_reconnect(tabId)`, `terminal_close(tabId)`, `clipboard_copy(text)`, `clipboard_read()`.

Rust → JS: `terminal_open` nhận thêm một `Channel` để stream **bytes thô** của shell; sự kiện `emit`: `tab:status {tabId, status, message}`, `tab:monitor {tabId, os, uptime, …}`, `dialog:request {kind: hostkey|password, tabId, …}`.

Phần xterm.js (`TerminalPane.ts`) giữ nguyên logic `terminal.js` v1: chờ font nạp xong mới `open()`, `ResizeObserver` + `fit()` debounce 100 ms, copy-on-select ở `mouseup`, chuột phải theo `RightClickAction` ⊕ Shift, `attachCustomKeyEventHandler` cho các phím tắt v1 mục 8.6, `term.paste()` cho dán.

### 8.3 SFTP
- Lệnh: `sftp_list(tabId, path)`, `sftp_mkdir`, `sftp_rename`, `sftp_remove(paths[], recursive)`, `sftp_chmod(paths[], mode, recursive)`, `sftp_upload(tabId, localPaths[], remoteDir)`, `sftp_download(tabId, remotePaths[], localDir)`, `sftp_open_in_editor(tabId, remotePath, editor?)`, `sftp_cancel(transferId)`.
- Hàng đợi truyền file chạy lần lượt, phát sự kiện `transfer:progress {id, name, bytes, total, speed}`; hủy thì xóa file dở.
- Mở bằng trình soạn thảo: tải về thư mục tạm `%LOCALAPPDATA%\SNTerm\edit\<tabId>\…`, mở bằng app mặc định hoặc `CustomEditorPath`, theo dõi file thay đổi (crate `notify`) → tự upload lại, giống v1.
- Lỗi quyền → "Không có quyền thực hiện thao tác này" (bảng `ErrorTranslator`).

---

## 9. Export / Import

Giữ nguyên v1 mục 5.4, 6.7, 6.8. Thêm yêu cầu kiểm thử chéo: trong `tests/fixtures/` để sẵn **file `.snterm` do v1 tạo thật** (2 chế độ) và test Rust phải import được, giải mã đúng mật khẩu; ngược lại file v2 tạo phải mở được bằng v1 (kiểm tra **[Tay]** một lần ở Giai đoạn 5).

---

## 10. Tính năng mới: Đồng bộ danh sách VM

### 10.1 Mục tiêu
- Dùng SN Term trên **nhiều máy Windows** (công ty, nhà, laptop) mà danh sách VM, nhóm, mật khẩu, key luôn **giống nhau**; thêm/sửa/xóa ở máy này thì máy kia tự có.
- **Không có máy chủ của SN Term.** Dữ liệu nằm trong **kho do người dùng chọn**, luôn **mã hóa đầu cuối** bằng **mật khẩu đồng bộ** mà chỉ người dùng biết.
- Hoạt động **ngầm**, không chặn giao diện; mất mạng thì bỏ qua, lần sau làm tiếp.
- Không bao giờ mất dữ liệu: trước lần đồng bộ đầu tiên luôn sao lưu; mọi lần gộp đều có nhật ký.

### 10.2 Kho đồng bộ (backend) — v2.0 hỗ trợ 2 loại

| Loại | Cách dùng | Lưu ở đâu | Phù hợp |
|---|---|---|---|
| **Thư mục** | Chọn một thư mục đã được OneDrive / Google Drive / Dropbox / ổ mạng đồng bộ sẵn (hoặc USB) | `<thư mục>\snterm-sync.vault` | Người dùng cá nhân, không có server |
| **SFTP tới một VM** | Chọn một VM trong danh sách (hoặc nhập host riêng) và đường dẫn trên máy chủ | mặc định `~/.snterm/sync.vault` | Có sẵn VM luôn bật; đồng bộ giữa các máy cùng truy cập được VM đó |

Cả hai đều chỉ cần 3 thao tác nguyên thủy: **đọc file**, **ghi file nguyên tử** (ghi `.tmp` rồi đổi tên), **đọc metadata** (có tồn tại, kích thước, mtime). Giao diện `trait SyncBackend` trong `sync/backend/mod.rs`; sau này thêm WebDAV/S3 chỉ cần cài thêm trait.

### 10.3 Định dạng kho (`snterm-sync.vault`)
Là **một file `.snterm` v1 hợp lệ** (mục 5.4 v1), **luôn ở chế độ có bảo vệ** (mật khẩu đồng bộ), cộng thêm vài trường mà v1 bỏ qua. Nhờ đó **bản v1 vẫn Import được file vault như một bản sao lưu** (nhập mật khẩu đồng bộ), và v2 không cần định dạng mới.

```json
{
  "format": "snterm-sessions", "version": 1, "exportedAt": "…", "appVersion": "2.0.0",
  "protection": { "kdf": "PBKDF2-SHA256", "iterations": 600000, "salt": "…", "check": {…} },
  "sync": {
    "revision": 42,
    "deviceId": "…guid của máy ghi lần cuối…", "deviceName": "LAPTOP-AN",
    "savedAt": "2026-10-03T07:30:00Z",
    "tombstones": [ { "id": "…", "deletedAt": "…" } ]
  },
  "sessions": [
    { "id": "…", "name": "web-01", "group": "Dev", "host": "…", "port": 22, "username": "…",
      "keyFileName": "id_ed25519", "secrets": {…},
      "updatedAt": "2026-10-02T09:12:00Z", "keyHash": "sha256-base64 của nội dung key (nếu có)" }
  ]
}
```

- `secrets` mã hóa như v1 (AES-256-GCM, AAD = id) và **luôn kèm `keyFileContent`** nếu VM dùng key và tùy chọn "Đồng bộ cả file key" bật (mặc định bật). `keyHash` để máy nhận biết key đã đổi mà không cần giải mã.
- `updatedAt`: thời điểm **nội dung** VM đổi (tên, nhóm, host, port, user, mật khẩu, key, passphrase). `LastConnectedAt`, `CollapsedGroups`, `KeyFilePath` (đường dẫn cục bộ) **không đồng bộ**.
- `tombstones`: VM đã xóa, giữ 180 ngày rồi dọn.
- `revision`: tăng 1 mỗi lần ghi; dùng để phát hiện ghi đè chéo (mục 10.4).

Phía cục bộ, `sessions.json` thêm `UpdatedAt` cho mỗi VM và danh sách `Deleted` (tombstone) ở envelope; `settings.json` thêm `Sync.LastRevision`, `Sync.LastSyncAt`, `Sync.DeviceId`.

### 10.4 Thuật toán một chu kỳ đồng bộ (`sync/engine.rs`)
```
1. Khóa (mutex) để không chạy 2 chu kỳ cùng lúc.
2. remote = backend.read()            // None nếu chưa có → bước 6 với local
3. Giải mã bằng mật khẩu đồng bộ (DPAPI mở ra từ settings). Sai mật khẩu → trạng thái "Cần mật khẩu đồng bộ", dừng.
4. merged = merge(local, remote):
     - với mỗi id trong (local ∪ remote ∪ tombstones):
         chỉ có local        → giữ
         chỉ có remote       → thêm (ghi key ra keys\ nếu có, mã hóa lại mật khẩu bằng DPAPI máy này)
         cả hai              → bản có updatedAt mới hơn thắng nguyên bản ghi; bằng nhau → giữ local
         tombstone vs bản ghi → cái nào có mốc thời gian mới hơn thắng
     - trùng "khác id, cùng host+port+user+tên" (hai máy cùng thêm tay) → gộp: giữ id có createdAt cũ hơn,
       nội dung theo updatedAt mới hơn, id còn lại thành tombstone.
5. Nếu merged ≠ local → sao lưu (lần đầu: sessions-before-sync-<ts>.json) → ghi sessions.json → làm mới danh sách.
6. Nếu merged ≠ remote → remote2 = backend.read_meta(); nếu revision đổi so với bước 2 → quay lại bước 2 (tối đa 3 lần);
   ngược lại backend.write(vault(merged, revision+1)).
7. Lưu LastRevision/LastSyncAt, phát sự kiện sync:status.
```
- So sánh "≠" trên **dữ liệu đã giải mã** để không ghi lại vault khi không có gì đổi (ổ đám mây không bị kích hoạt vô ích).
- Với backend Thư mục, ngoài `snterm-sync.vault` còn quét các file `snterm-sync*.vault` khác (bản "conflicted copy" do OneDrive/Dropbox tạo) → gộp luôn rồi xóa file phụ sau khi ghi thành công.
- Lệch giờ: nếu `savedAt` của remote đi trước giờ máy > 5 phút → cảnh báo một lần "Giờ máy có thể sai, đồng bộ có thể chọn nhầm bản mới/cũ".

### 10.5 Khi nào chạy
- Khởi động app (sau 3 s), sau mỗi thay đổi danh sách (debounce 10 s), định kỳ mỗi `Sync.IntervalMinutes` (mặc định 15), bấm nút ⟳, và trước khi đóng app (chờ tối đa 5 s).
- Lỗi mạng → ghi log, status bar "Chưa đồng bộ (ngoại tuyến)", thử lại lần sau; **không** hiện hộp thoại.

### 10.6 Bảo mật
- Mật khẩu đồng bộ ≥ 8 ký tự (khuyên 12), có thanh độ mạnh; lưu cục bộ bằng DPAPI để chạy ngầm; không bao giờ ghi log.
- Vault chứa đủ để đăng nhập mọi VM → README nói rõ: ai có vault **và** mật khẩu đồng bộ thì vào được mọi VM; dùng mật khẩu mạnh; với backend SFTP, đặt quyền `0600` cho file vault và `0700` cho thư mục.
- Khóa và dữ liệu rõ dùng `zeroize` ngay sau khi dùng.
- Đổi mật khẩu đồng bộ: mã hóa lại vault, tăng revision; máy khác sẽ báo "Cần mật khẩu đồng bộ mới".
- `known_hosts` **không** đồng bộ (như quy tắc Import v1); có thể thêm tùy chọn ở bản sau.

### 10.7 Giao diện
**Hộp thoại thiết lập lần đầu (`SyncSetup`)**, 3 bước:
1. Chọn loại kho: (•) Thư mục [Chọn…] / ( ) SFTP tới VM [chọn VM ▾] đường dẫn `[~/.snterm/sync.vault]`, nút **Kiểm tra**.
2. Mật khẩu đồng bộ: nếu kho **đã có vault** → nhập mật khẩu (sai → báo, thử lại); nếu **chưa có** → đặt mật khẩu mới (nhập 2 lần). `[x] Đồng bộ cả file key`.
3. **Xem trước lần gộp đầu** (`SyncPreview`): "Sẽ thêm X VM từ kho, cập nhật Y, xóa Z, gửi lên kho W" kèm danh sách; **[Bắt đầu đồng bộ]** / [Hủy]. Hủy → không đổi gì.

**Tab "Đồng bộ" trong Cài đặt**: bật/tắt, loại kho và đường dẫn (nút Đổi…), [Đổi mật khẩu đồng bộ…], chu kỳ (phút), [x] file key, [Đồng bộ ngay], [Xem nhật ký], trạng thái lần cuối, và **[Ngắt đồng bộ]** (giữ dữ liệu cục bộ, chỉ dừng).

**Nút ⟳ trên toolbar và góc phải status bar** như mục 6.

### 10.8 Kiểm thử bắt buộc (`sync/merge.rs`, `sync/engine.rs`, backend giả trong RAM)
- Thêm ở A → xuất hiện ở B; sửa ở B → về A; xóa ở A → mất ở B; xóa ở A nhưng B sửa sau đó → B thắng (VM quay lại).
- Hai máy cùng sửa một VM → bản `updatedAt` mới hơn thắng; bằng nhau → giữ local, không dao động qua lại.
- Hai máy cùng thêm tay một VM giống nhau → sau 2 chu kỳ chỉ còn 1 VM, id ổn định.
- Ghi chéo: A đọc rev 5, B ghi rev 6, A ghi → A phải phát hiện, gộp lại rev 6 rồi mới ghi rev 7.
- Vault bị hỏng 1 byte → chu kỳ dừng, báo lỗi, **không** sửa dữ liệu cục bộ.
- Sai mật khẩu → không đổi gì, trạng thái "Cần mật khẩu đồng bộ".
- Không có gì đổi → không gọi `backend.write`.
- File "conflicted copy" được gộp và dọn.
- Vault do v2 tạo **Import được bằng v1** (kiểm tra bằng test C# trong `tests/SNTerm.Tests` với fixture v2) và vault chứa **không** mật khẩu dạng chữ thường.
- Key file đồng bộ về đúng `keys\<id>_<tên>`, `KeyFilePath` trỏ đúng; tắt tùy chọn key → VM nhận về đánh dấu ⚠.

---

## 11. Các giai đoạn triển khai

### Giai đoạn 0 — Khung Tauri + bố cục
- Tạo `v2/` với Tauri 2, Svelte 5, Vite, TypeScript; cấu hình `tauri.conf.json` (một cửa sổ, `data_directory` WebView2 tại `%LOCALAPPDATA%\SNTerm\WebView2`, tắt devtools ở release, icon từ `src/SNTerm/Assets/app.ico`).
- `theme.css` từ `ThemeManager.cs`; `i18n/vi.json`, `en.json` từ `LocalizationManager.cs` (viết script chuyển tự động, kiểm tra lại tay).
- Dựng `App.svelte` đúng bố cục v1 với dữ liệu giả, splitter kéo được, theme sáng/tối, đổi ngôn ngữ.
- Profile release tối ưu kích thước (mục 5.2).

**Hoàn thành khi:** `cargo tauri build` ra 1 exe ≤ 10 MB; **[Tay]** mở app thấy bố cục giống v1 đặt cạnh nhau (chụp 2 ảnh so sánh), kéo được splitter, đổi theme/ngôn ngữ.

### Giai đoạn 1 — Dữ liệu tương thích v1 + quản lý VM
- `store/*`, `crypto/dpapi.rs` (cùng entropy), sao lưu hằng ngày, khôi phục file hỏng.
- Danh sách VM đầy đủ (nhóm, Gần đây, tìm kiếm, chọn nhiều, menu, phím tắt, ⚠ key, icon OS theo `LiveStatus`/thông tin monitor), form Thêm/Sửa VM, Nhân bản, nhóm.
- Test Rust: đọc được `sessions.json` **do v1 tạo thật** (fixture không chứa mật khẩu thật), ghi ra rồi v1 đọc lại được (so sánh JSON), DPAPI round-trip.

**Hoàn thành khi:** test qua; **[Tay]** chạy v2 trên máy đang có v1 → thấy đủ VM, đúng nhóm; thêm/sửa/xóa rồi mở lại **v1** vẫn thấy đúng.

### Giai đoạn 2 — SSH và terminal
- `ssh/*` theo mục 8.1, `HostKeyDialog`, `PasswordPromptDialog`, hàng đợi hộp thoại, keyboard-interactive, key OpenSSH/PKCS#8/PPK, mật khẩu đã lưu sai → hỏi lại và cập nhật.
- `TerminalHost` nhiều xterm.js, Channel nhị phân, gom 16 ms/64 KB, resize, WebGL, giữ nội dung khi chuyển tab, kết nối lại bằng R/Enter.
- Thanh tab đầy đủ (mục 6.6 v1), mở nhiều VM song song, nút "Kiểm tra kết nối" trong form, thông tin máy chủ → icon OS + tooltip.

**Hoàn thành khi:** toàn bộ mục **[Tay]** của Giai đoạn 2 v1 qua, cộng thêm: mở 10 tab → RAM tăng ≤ 80 MB so với 1 tab; `netstat` thấy 1 kết nối/tab; gõ tiếng Việt bằng Unikey/EVKey trong `nano` đúng dấu.

### Giai đoạn 3 — Copy / Paste / phím tắt
Như Giai đoạn 3 v1 (clipboard có retry 5 lần × 50 ms, xác nhận dán nhiều dòng, menu chuột phải, `Ctrl+C` vẫn dừng được `ping`).

### Giai đoạn 4 — SFTP
Toàn bộ mục 9 v1 + chmod, mở bằng trình soạn thảo và tự upload lại, tìm nhanh bằng bàn phím (chuyển `SftpTypeAheadTests.cs` sang vitest), danh sách ảo hóa.

**Hoàn thành khi:** mục **[Tay]** Giai đoạn 4 v1 qua; thư mục 10.000 file cuộn mượt; chmod đệ quy đúng; sửa file bằng Notepad rồi lưu → file trên VM đổi.

### Giai đoạn 5 — Export / Import
`crypto/snterm_file.rs`, `import/mobaxterm.rs`, hộp thoại Export/Import, kéo thả, mở file `.snterm` bằng nhấp đúp (single-instance + tham số dòng lệnh). Toàn bộ test bắt buộc của Giai đoạn 5 v1 chuyển sang Rust + test chéo với fixture v1 thật.

**Hoàn thành khi:** test qua; **[Tay]** file v2 export mở được bằng v1 và ngược lại.

### Giai đoạn 6 — Cài đặt và hoàn thiện
Settings đầy đủ, áp dụng ngay cho mọi tab; nhớ cửa sổ; log; bắt panic/lỗi toàn cục (không tự tắt); hỏi khi đóng còn tab kết nối; kiểm tra WebView2 Runtime lúc khởi động (dùng bootstrapper của Tauri hoặc thông báo kèm link như v1).

### Giai đoạn 7 — Đồng bộ danh sách VM (mục 10)
- `sync/merge.rs` + test (có thể làm song song ngay sau Giai đoạn 1 vì chỉ là logic thuần).
- `sync/vault.rs`, backend Thư mục, backend SFTP (dùng lại `ssh/*`), `sync/engine.rs`, lịch chạy, trạng thái.
- Hộp thoại `SyncSetup`, `SyncPreview`, tab Đồng bộ trong Cài đặt, nút ⟳, status bar.
- Test C# trong `tests/SNTerm.Tests` xác nhận v1 Import được vault v2.

**Hoàn thành khi:** test mục 10.8 qua; **[Tay]**
- Máy A bật đồng bộ qua thư mục OneDrive; máy B (hoặc tài khoản Windows khác) bật cùng thư mục, nhập mật khẩu đồng bộ → thấy đủ VM, nhấp đúp vào thẳng (chỉ hỏi host key).
- Thêm VM ở A → ≤ 1 phút sau B có (sau khi OneDrive đồng bộ file); xóa ở B → A mất.
- Rút mạng ở A, sửa VM, cắm lại → tự đồng bộ, không hộp thoại nào.
- Backend SFTP tới một VM: hai máy cùng đồng bộ qua VM đó hoạt động; file trên VM có quyền `0600`.
- Lấy vault trên OneDrive, mở bằng **v1** → Import được, nhập mật khẩu đồng bộ → đủ VM.

### Giai đoạn 8 — Đóng gói và tài liệu
- `cargo tauri build`: exe portable + bộ cài (NSIS của Tauri, hoặc cập nhật `SNTerm.iss` trỏ sang exe mới; chọn một, bỏ cái kia). Liên kết đuôi `.snterm`.
- README.md: cập nhật yêu cầu (chỉ cần WebView2), cách chuyển từ v1 (không cần làm gì), **hướng dẫn Đồng bộ** (2 loại kho, cảnh báo bảo mật, xử lý sự cố: sai mật khẩu, conflicted copy, lệch giờ), bảng so sánh hiệu năng (từ Giai đoạn 9).
- Đánh dấu `src/SNTerm/` (v1) là "legacy"; xóa ở phiên bản sau khi v2 ổn định.

### Giai đoạn 9 — Đo và chốt
Đo các chỉ tiêu mục 1 trên cùng một máy cho v1 và v2 (3 lần lấy trung bình), ghi vào README. Chỉ tiêu nào không đạt → tối ưu tiếp hoặc ghi rõ lý do.

---

## 12. Rủi ro và cách xử lý

| Rủi ro | Mức | Xử lý |
|---|---|---|
| `russh` thiếu/khác API (keyboard-interactive, `window_change`, keepalive) ở phiên bản đang dùng | Trung bình | Đọc docs của phiên bản khóa trong `Cargo.lock`; làm **spike 1–2 ngày** ngay đầu Giai đoạn 2 với VM thật (password, key, ppk, resize, htop). Thất bại → chuyển lõi sang **Go + Wails** (mục 3.1), giữ nguyên phần giao diện và thiết kế |
| Key `.ppk` (PuTTY) không có crate sẵn | Trung bình | Viết `crypto/ppk.rs` cho PPK v2 (SHA-1/AES-CBC) và v3 (Argon2id) theo tài liệu PuTTY, test với key mẫu; đồng thời hộp thoại gợi ý chuyển sang OpenSSH bằng `puttygen` nếu đọc lỗi |
| Tauri Channel chưa hỗ trợ payload thô ở phiên bản dùng | Thấp | Fallback Base64 (như v1), vẫn đạt chỉ tiêu RAM vì vẫn 1 WebView |
| Tiếng Việt IME trong WebView2 | Thấp | Cùng engine WebView2 như v1 nên hành vi giống; kiểm tra **[Tay]** sớm ở Giai đoạn 2 |
| Kéo thả từ Explorer trong Tauri chỉ cấp đường dẫn, không cấp stream | Thấp | Đủ dùng: Rust tự đọc file từ đường dẫn |
| Đồng bộ qua ổ đám mây tạo "conflicted copy" / đồng bộ trễ | Trung bình | Gộp file phụ (10.4), revision + đọc lại trước khi ghi, README giải thích độ trễ phụ thuộc ổ đám mây |
| Lệch giờ giữa máy → chọn nhầm bản thắng | Thấp | Cảnh báo khi lệch > 5 phút; quy tắc hòa ổn định; sao lưu trước lần gộp đầu; nhật ký gộp để khôi phục |
| Vault lộ + mật khẩu yếu | Trung bình | Bắt buộc ≥ 8 ký tự, thanh độ mạnh, cảnh báo trong README và trong hộp thoại thiết lập |
| Antivirus/SmartScreen với exe Rust chưa ký | Thấp | Như v1; ký số là tùy chọn |
| Người dùng chạy song song v1 và v2 trên cùng máy ghi đè `sessions.json` | Thấp | Cả hai ghi nguyên tử, v2 đọc lại trước khi ghi; README khuyên chỉ dùng một bản |

---

## 13. Kiểm thử nhanh khi chưa có VM
Như v1 mục 12 (WSL + `openssh-server`). Để thử đồng bộ chỉ với một máy: tạo **hai tài khoản Windows** hoặc chạy v2 với biến môi trường `SNTERM_DATA_DIR=<thư mục khác>` (v2 hỗ trợ biến này, chỉ dùng cho kiểm thử) và trỏ cả hai vào cùng một thư mục đồng bộ.
