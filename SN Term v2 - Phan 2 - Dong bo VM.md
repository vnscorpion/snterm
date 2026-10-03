# SN Term v2 — Phần 2: Đồng bộ danh sách VM giữa nhiều máy

> Thêm tính năng **Đồng bộ danh sách VM** (kèm nhóm, mật khẩu, key) giữa nhiều máy Windows, **không cần máy chủ của SN Term**, mã hóa đầu cuối, chạy ngầm.
>
> Tài liệu này **độc lập với việc chuyển ngôn ngữ** (Phần 1). Nó được viết để làm **ngay trên bản v1 hiện tại (C# / WPF)**, vì phần chuyển ngôn ngữ mất nhiều thời gian hơn. Định dạng kho và thuật toán gộp **không phụ thuộc ngôn ngữ**, nên khi v2 Rust xong chỉ cần chuyển mã theo bảng ánh xạ ở mục 12.

---

## 0. Hướng dẫn cho AI/CLI thực hiện kế hoạch này

1. Đọc `SN Term.md` (v1) mục 5 (dữ liệu), 5.4 (file `.snterm`), 6.1 (bố cục) trước. Kho đồng bộ **dùng lại định dạng `.snterm`** nên không phát minh định dạng mới.
2. Làm **tuần tự từng giai đoạn** (mục 11). Sau mỗi giai đoạn: `dotnet build` không lỗi/cảnh báo mới, `dotnet test` qua, `git commit` với thông điệp `Đồng bộ Giai đoạn N: <tóm tắt>`.
3. **Không đổi bố cục** cửa sổ chính. Chỉ thêm **một nút** trên thanh công cụ và **một ô chữ** ở góc phải thanh trạng thái (mục 7).
4. **Không bao giờ làm mất dữ liệu cục bộ**: trước lần gộp đầu tiên luôn sao lưu; mọi lần gộp ghi nhật ký; lỗi ở bất kỳ bước nào → dừng, **không** sửa `sessions.json`.
5. Không ghi mật khẩu VM, passphrase, **mật khẩu đồng bộ** ra log, ra file dạng chữ thường, hay ra kho ở dạng chưa mã hóa.
6. Mọi thao tác mạng/đĩa của đồng bộ chạy nền (`async/await`, `Task.Run`), không chặn UI; lỗi mạng **không** hiện hộp thoại, chỉ đổi trạng thái.
7. Người dùng không phải lập trình viên: báo cáo ngắn gọn, liệt kê mục **[Tay]** cần tự thử.

---

## 1. Mục tiêu

- Dùng SN Term trên **nhiều máy** (công ty, nhà, laptop): thêm/sửa/xóa VM ở máy này thì máy kia **tự có**, kể cả mật khẩu và file key → nhấp đúp vào thẳng (chỉ hỏi host key lần đầu, như Import).
- **Kho đồng bộ do người dùng chọn**: thư mục đã được OneDrive/Google Drive/Dropbox đồng bộ, ổ mạng, hoặc **một VM có sẵn** qua SFTP. Không có dịch vụ riêng.
- **Mã hóa đầu cuối** bằng **mật khẩu đồng bộ** mà chỉ người dùng biết; kho chỉ chứa dữ liệu đã mã hóa.
- Chạy **ngầm** khi khởi động, sau mỗi thay đổi, định kỳ, và khi bấm nút. Mất mạng → bỏ qua, lần sau làm tiếp.
- Tương thích: file kho **Import được bằng bản SN Term không có tính năng đồng bộ** (như một bản sao lưu `.snterm`).

**Ngoài phạm vi:** máy chủ đồng bộ riêng, đồng bộ cài đặt giao diện, đồng bộ `known_hosts` (có thể thêm sau), đồng bộ lịch sử terminal, WebDAV/S3/Git (chừa sẵn interface, chưa làm).

---

## 2. Kho đồng bộ (backend) — v1 hỗ trợ 2 loại

| Loại | Cách dùng | Lưu ở đâu | Phù hợp |
|---|---|---|---|
| **Thư mục** | Chọn thư mục đã được OneDrive / Google Drive / Dropbox / ổ mạng đồng bộ sẵn (hoặc USB) | `<thư mục>\snterm-sync.vault` | Cá nhân, không có server |
| **SFTP tới một VM** | Chọn một VM trong danh sách (dùng luôn thông tin đăng nhập đã lưu) và đường dẫn trên máy chủ | mặc định `~/.snterm/sync.vault` | Có VM luôn bật; các máy đều truy cập được VM đó |

Interface (`Services/Sync/ISyncBackend.cs`):

```csharp
public interface ISyncBackend
{
    string DisplayName { get; }                                  // "OneDrive\SNTerm" hoặc "web-01:~/.snterm"
    Task<SyncBlob?> ReadAsync(CancellationToken ct);             // null nếu chưa có kho
    Task WriteAsync(byte[] content, CancellationToken ct);       // ghi nguyên tử: .tmp rồi đổi tên
    Task<SyncBlobMeta?> ReadMetaAsync(CancellationToken ct);     // tồn tại? kích thước, mtime (không tải nội dung)
    Task<IReadOnlyList<SyncBlob>> ReadConflictCopiesAsync(CancellationToken ct); // bản "conflicted copy" (mục 4.3)
    Task DeleteConflictCopiesAsync(IEnumerable<string> names, CancellationToken ct);
    Task<string?> TestAsync(CancellationToken ct);               // null = OK, khác = lý do lỗi tiếng Việt
}
public record SyncBlobMeta(long Length, DateTime LastWriteUtc);
public record SyncBlob(string Name, byte[] Content, SyncBlobMeta Meta);
```

- `FolderSyncBackend`: `File.ReadAllBytes` với retry 5 lần × 200 ms khi file đang bị ổ đám mây khóa; ghi `snterm-sync.vault.tmp` rồi `File.Move(..., overwrite: true)`.
- `SftpSyncBackend`: dùng `SshConnectionFactory` + `SftpClient` **riêng** (không dính tab terminal), kết nối khi cần rồi ngắt; host key kiểm tra qua `KnownHostsStore` như tab bình thường (chưa tin cậy → bỏ qua chu kỳ này, trạng thái "Cần xác nhận host key — mở VM một lần"); tạo thư mục nếu thiếu, `chmod 700` thư mục, `chmod 600` file; ghi `.tmp` rồi `RenameFile`.

---

## 3. Định dạng kho `snterm-sync.vault`

Là **một file `.snterm` v1 hợp lệ** (v1 mục 5.4), **luôn ở chế độ có bảo vệ** (mật khẩu đồng bộ), **luôn kèm nội dung file key** (nếu tùy chọn bật, mặc định bật), cộng thêm vài trường mà bộ Import hiện tại bỏ qua. Nhờ đó bản không có tính năng đồng bộ vẫn **Import** được file kho như bản sao lưu.

```json
{
  "format": "snterm-sessions", "version": 1, "exportedAt": "…", "appVersion": "1.1.0",
  "protection": { "kdf": "PBKDF2-SHA256", "iterations": 600000, "salt": "…", "check": {…} },
  "sync": {
    "revision": 42,
    "deviceId": "guid máy ghi lần cuối", "deviceName": "LAPTOP-AN",
    "savedAt": "2026-10-03T07:30:00Z",
    "tombstones": [ { "id": "…", "deletedAt": "2026-10-01T02:00:00Z" } ]
  },
  "sessions": [
    { "id": "…", "name": "web-01", "group": "Dev", "host": "10.0.0.5", "port": 22, "username": "ubuntu",
      "keyFileName": "id_ed25519", "secrets": { "nonce": "…", "cipherText": "…", "tag": "…" },
      "updatedAt": "2026-10-02T09:12:00Z", "keyHash": "sha256 base64 của nội dung key, null nếu không có" }
  ]
}
```

- `secrets` mã hóa đúng như `SessionExporter` (AES-256-GCM, AAD = chuỗi `id`), payload `{password, passphrase, keyFileContent}`.
- `updatedAt`: thời điểm **nội dung** VM đổi (tên, nhóm, host, port, user, mật khẩu, passphrase, key). **Không** đổi khi chỉ cập nhật `LastConnectedAt`.
- Không đồng bộ: `LastConnectedAt`, `KeyFilePath` (đường dẫn cục bộ; máy nhận tự ghi key ra `keys\` và gán lại), `CollapsedGroups`.
- `tombstones`: VM đã xóa, giữ **180 ngày** rồi dọn.
- `revision`: tăng 1 mỗi lần ghi, dùng phát hiện ghi đè chéo (mục 4.2).

### 3.1 Thay đổi dữ liệu cục bộ (tương thích ngược)

`sessions.json` (envelope `SessionFileEnvelope`):
```json
{ "version": 1, "sessions": [ { …như cũ…, "UpdatedAt": "…" } ],
  "Deleted": [ { "Id": "…", "DeletedAt": "…" } ] }
```
- `SessionInfo.UpdatedAt` (`DateTime`): khi đọc file cũ không có → gán = `CreatedAt`. Mọi nơi sửa nội dung VM phải đặt `UpdatedAt = DateTime.UtcNow`: `SessionEditorViewModel` (Lưu), `SessionListViewModel` (nhân bản, đổi nhóm, đổi tên nhóm, bỏ nhóm), `SessionImporter` (Ghi đè / thêm), `TerminalTabViewModel` khi cập nhật mật khẩu đã lưu sai (dòng ~427/491). **Không** đặt khi chỉ đổi `LastConnectedAt`.
- Xóa VM → thêm tombstone vào `Deleted` thay vì chỉ bỏ khỏi danh sách. Bản cũ bỏ qua `Deleted` nên quay lại bản cũ vẫn chạy.

`settings.json` thêm khối:
```json
"Sync": { "Enabled": false, "BackendType": "Folder", "FolderPath": "", "SftpSessionId": null,
          "SftpRemotePath": "~/.snterm/sync.vault", "EncryptedSyncPassword": null,
          "IntervalMinutes": 15, "IncludeKeyFiles": true,
          "DeviceId": "guid tạo lần đầu", "DeviceName": "tên máy",
          "LastRevision": 0, "LastSyncAt": null, "LastError": null }
```
`EncryptedSyncPassword` mã hóa bằng `SecretProtector` (DPAPI) để chạy ngầm không hỏi lại.

---

## 4. Thuật toán

### 4.1 Một chu kỳ đồng bộ (`SyncEngine.RunOnceAsync`)
```
1. SemaphoreSlim(1): đang chạy thì bỏ qua lần gọi mới (đánh dấu "chạy lại sau khi xong").
2. remote = backend.ReadAsync()            // null → chưa có kho → đi tiếp với remote rỗng, revision 0
   + conflictCopies = backend.ReadConflictCopiesAsync()
3. Giải mã remote (và từng conflict copy) bằng mật khẩu đồng bộ:
   - sai mật khẩu (check block thất bại) → trạng thái "Cần mật khẩu đồng bộ", DỪNG, không đổi gì
   - file hỏng (JSON/AES lỗi) → trạng thái "Kho bị hỏng", DỪNG; conflict copy hỏng → bỏ qua file đó, ghi log
4. merged = SyncMerger.Merge(local, remote, conflictCopies…)   // thuần, mục 4.2
5. Nếu merged.Local ≠ local:
   - lần đầu (LastRevision == 0): backups\sessions-before-sync-<yyyyMMdd-HHmmss>.json
   - với mỗi VM nhận từ kho: mật khẩu/passphrase → SecretProtector.Encrypt (DPAPI máy này);
     key: nếu keyHash khác file hiện có → ghi keys\<id>_<keyFileName> (NTFS chỉ user hiện tại), gán KeyFilePath
   - SessionStore.Save(merged) (qua đường "không kích hoạt đồng bộ lại")
   - làm mới danh sách VM trên UI (Dispatcher)
6. Nếu merged.Remote ≠ remote (so sánh trên dữ liệu đã giải mã, bỏ qua thứ tự):
   - meta = backend.ReadMetaAsync(); nếu khác meta ở bước 2 (kích thước/mtime) → quay lại bước 2 (tối đa 3 lần)
   - backend.WriteAsync(vault(merged, revision = max(remote.revision, LastRevision) + 1))
   - xóa các conflict copy đã gộp
7. Lưu LastRevision, LastSyncAt, LastError = null → sự kiện SyncStatusChanged
Mọi exception → LastError = thông báo tiếng Việt (ErrorTranslator), log chi tiết, trạng thái lỗi; không ném lên UI.
```

### 4.2 Gộp (`SyncMerger.Merge`) — hàm thuần, không I/O
Với mỗi `id` trong hợp của (local, remote, mọi conflict copy, tombstone hai phía):

| Tình huống | Kết quả |
|---|---|
| Chỉ local có, remote không có và **không có tombstone** | giữ local (VM mới thêm ở máy này) |
| Chỉ remote có, local không có và không có tombstone cục bộ | thêm vào local |
| Cả hai có | bản `updatedAt` **mới hơn** thắng nguyên bản ghi; bằng nhau → giữ local (không dao động) |
| Bản ghi vs tombstone | mốc thời gian nào **mới hơn** thắng: `deletedAt` mới hơn → xóa; `updatedAt` mới hơn → VM "sống lại" |
| Hai id khác nhau nhưng **cùng host + port + user + tên** (hai máy cùng thêm tay) | gộp làm một: giữ id có `createdAt` cũ hơn, nội dung theo `updatedAt` mới hơn, id còn lại thành tombstone |

Kết quả gồm `Local` (danh sách mới cho máy này), `Remote` (danh sách mới cho kho), `Tombstones` (đã dọn > 180 ngày), và `Report` (số thêm/cập nhật/xóa/gửi lên) để hiện ở bước xem trước.

Lệch giờ: nếu `remote.sync.savedAt` đi trước giờ máy > 5 phút → cảnh báo **một lần** trên status bar "Giờ máy có thể sai; đồng bộ có thể chọn nhầm bản mới/cũ".

### 4.3 Bản "conflicted copy" của ổ đám mây
Khi hai máy ghi gần như cùng lúc, OneDrive/Dropbox/Google Drive tạo file phụ (`snterm-sync-PCNAME.vault`, `snterm-sync (conflicted copy …).vault`, `snterm-sync (1).vault`). `FolderSyncBackend` trả về mọi file khớp `snterm-sync*.vault` trừ file chính; engine gộp hết rồi xóa sau khi ghi thành công. Backend SFTP không có hiện tượng này (đổi tên nguyên tử trên server).

### 4.4 Khi nào chạy (`SyncScheduler`)
- Khởi động app: sau 3 s.
- Sau mỗi thay đổi danh sách (`SessionStore.Changed` event): debounce 10 s.
- Định kỳ mỗi `IntervalMinutes` (mặc định 15, tối thiểu 5).
- Bấm nút ⟳ hoặc "Đồng bộ ngay".
- Trước khi đóng app: nếu có thay đổi chưa đẩy, chạy một lần, chờ tối đa 5 s.
- Máy đang ngủ/không mạng: lỗi được nuốt, trạng thái "Chưa đồng bộ (ngoại tuyến)", thử lại lần sau.

---

## 5. Bảo mật

- Mật khẩu đồng bộ ≥ 8 ký tự (khuyên ≥ 12), nhập 2 lần, thanh độ mạnh (dùng lại của ExportDialog).
- Kho chứa đủ để đăng nhập mọi VM → hộp thoại thiết lập và README ghi rõ: **ai có file kho và mật khẩu đồng bộ sẽ vào được mọi VM**; không gửi file và mật khẩu qua cùng một kênh.
- Khóa dẫn xuất và dữ liệu rõ: `CryptographicOperations.ZeroMemory` ngay sau dùng (như Exporter/Importer).
- Backend SFTP: `chmod 700` thư mục, `chmod 600` file kho; dùng host key đã tin cậy, không tự tin cậy.
- Đổi mật khẩu đồng bộ: mã hóa lại kho, `revision + 1`; máy khác sẽ báo "Cần mật khẩu đồng bộ" và hỏi khi người dùng nhấp vào trạng thái.
- "Ngắt đồng bộ" trên một máy: xóa `EncryptedSyncPassword` cục bộ, giữ dữ liệu VM; có tùy chọn "Xóa file kho" (hỏi xác nhận, chỉ khi là máy cuối cùng).
- Log (`%LOCALAPPDATA%\SNTerm\logs\sync-YYYYMMDD.log`): thời điểm, backend, revision, số thêm/cập nhật/xóa, lỗi. **Không** tên file key, không mật khẩu.

---

## 6. Cấu trúc mã (trên v1 C#)

```
src/SNTerm/
├─ Models/
│  ├─ SessionInfo.cs            // + UpdatedAt
│  ├─ SessionTombstone.cs       // Id, DeletedAt
│  ├─ SyncSettings.cs           // khối "Sync" trong AppSettings
│  └─ ExportFile.cs             // + ExportSyncBlock (sync), ExportSessionItem.UpdatedAt, KeyHash
├─ Services/
│  ├─ SessionStore.cs           // + Deleted, event Changed, Save(…, raiseChanged: false)
│  ├─ SntermCrypto.cs           // TÁCH từ SessionExporter/Importer: DeriveKey, Encrypt/DecryptBlock, BuildCheck/VerifyCheck
│  └─ Sync/
│     ├─ ISyncBackend.cs
│     ├─ FolderSyncBackend.cs
│     ├─ SftpSyncBackend.cs
│     ├─ SyncVault.cs           // ExportFile ⇄ byte[] (dùng SntermCrypto), đọc/ghi key ra keys\
│     ├─ SyncMerger.cs          // thuần, test được
│     ├─ SyncEngine.cs          // RunOnceAsync, trạng thái, sự kiện
│     ├─ SyncScheduler.cs       // debounce / định kỳ / khởi động / đóng app
│     └─ SyncLog.cs
├─ ViewModels/
│  ├─ MainViewModel.cs          // + SyncCommand, SyncStatusText, SyncStatusKind
│  └─ SyncSetupViewModel.cs
└─ Views/
   ├─ MainWindow.xaml           // + nút ⟳ Đồng bộ, ô trạng thái bên phải status bar
   ├─ SyncSetupDialog.xaml      // 3 bước (mục 7)
   ├─ SyncPreviewDialog.xaml    // xem trước lần gộp đầu
   └─ SettingsDialog.xaml       // + mục "Đồng bộ"
```

`SessionExporter`/`SessionImporter` sau khi tách `SntermCrypto` phải **không đổi hành vi** (test Export/Import hiện có vẫn qua nguyên).

---

## 7. Giao diện (giữ bố cục v1)

```
│ [▶ Kết nối] [+ Thêm VM] [⇪ Export] [⇩ Import] [⟳ Đồng bộ] [⚙ Cài đặt]   │   ← thêm 1 nút, cùng ToolbarButtonStyle
…
│ Đã kết nối web-01 (10.0.0.5:22)                  Đồng bộ: 14:32 ✓  (OneDrive) │   ← TextBlock bên phải status bar
```

- **Nút ⟳ Đồng bộ**: chưa bật → mở `SyncSetupDialog`; đã bật → đồng bộ ngay; icon xoay khi đang chạy; đỏ khi lỗi (tooltip = `LastError`).
- **Status bar phải**: `Đồng bộ: HH:mm ✓` / `Đang đồng bộ…` / `Chưa đồng bộ (ngoại tuyến)` / `Cần mật khẩu đồng bộ` / `Lỗi đồng bộ: <lý do>`; ẩn khi tắt. Nhấp → mở mục Đồng bộ trong Cài đặt (hoặc hộp nhập mật khẩu nếu đang "Cần mật khẩu").
- **`SyncSetupDialog`** 3 bước:
  1. Loại kho: (•) Thư mục `[C:\Users\An\OneDrive\SNTerm] [Chọn…]` / ( ) SFTP tới VM `[web-01 ▾]` đường dẫn `[~/.snterm/sync.vault]` — nút **[Kiểm tra]** gọi `TestAsync`.
  2. Mật khẩu đồng bộ: kho **đã có** → nhập (sai → "Sai mật khẩu đồng bộ", thử lại); kho **chưa có** → đặt mới 2 lần + thanh độ mạnh. `[x] Đồng bộ cả file key`. Cảnh báo bảo mật (mục 5).
  3. **Xem trước** (`SyncPreviewDialog`): "Sẽ thêm X VM từ kho, cập nhật Y, xóa Z, gửi lên kho W" kèm bảng tên VM + hành động; **[Bắt đầu đồng bộ]** / [Hủy]. Hủy → không đổi gì (kể cả settings).
- **Cài đặt → mục "Đồng bộ"** (thêm `GroupBox` vào `SettingsDialog`): bật/tắt; loại kho và đường dẫn + [Đổi…]; [Đổi mật khẩu đồng bộ…]; chu kỳ (phút); [x] file key; [Đồng bộ ngay]; [Xem nhật ký]; dòng trạng thái lần cuối; **[Ngắt đồng bộ]**.
- Chuỗi mới thêm vào `LocalizationManager` cả `vi` và `en` (`Str_Sync*`).

---

## 8. Thông báo lỗi (thêm vào `ErrorTranslator`)

| Tình huống | Thông báo |
|---|---|
| Thư mục không tồn tại / không ghi được | Không truy cập được thư mục đồng bộ: `<đường dẫn>` |
| SFTP không kết nối / sai đăng nhập | Dùng lại thông báo SSH của v1 + "(kho đồng bộ)" |
| Host key chưa tin cậy | Chưa xác nhận host key của `<VM>` — hãy mở VM này một lần |
| Sai mật khẩu đồng bộ | Sai mật khẩu đồng bộ. Nhấp vào đây để nhập lại. |
| Kho hỏng | File kho đồng bộ bị hỏng. Không thay đổi dữ liệu trên máy này. |
| Kho do phiên bản mới hơn tạo (`version > 1`) | File kho được tạo bởi phiên bản SN Term mới hơn, hãy cập nhật ứng dụng |
| Lệch giờ | Giờ máy có thể sai; đồng bộ có thể chọn nhầm bản mới/cũ |

---

## 9. Kiểm thử bắt buộc (xUnit, `tests/SNTerm.Tests/Sync*.cs`, backend giả `InMemorySyncBackend`)

**`SyncMerger` (thuần):**
- Thêm ở A → B có; sửa ở B → A có; xóa ở A → B mất; xóa ở A nhưng B sửa **sau** → VM sống lại ở cả hai.
- Hai máy cùng sửa một VM → `updatedAt` mới hơn thắng; bằng nhau → giữ local, chạy thêm 3 chu kỳ không đổi gì.
- Hai máy cùng thêm tay VM giống nhau (khác id) → sau 2 chu kỳ còn 1 VM, id ổn định, không mất mật khẩu.
- Tombstone > 180 ngày bị dọn; < 180 ngày giữ.
- Tên tiếng Việt, nhóm tiếng Việt giữ nguyên.

**`SyncEngine` + backend giả:**
- Kho chưa có → tạo kho revision 1 với toàn bộ local.
- Ghi chéo: A đọc rev 5; B ghi rev 6; A ghi → A phát hiện meta đổi, gộp lại, ghi rev 7 chứa cả thay đổi của B.
- Không có gì đổi → `WriteAsync` **không** được gọi.
- Sai mật khẩu → không gọi `Save`, trạng thái "Cần mật khẩu đồng bộ".
- Kho hỏng 1 byte → dừng, không gọi `Save`.
- Conflict copy được gộp và xóa sau khi ghi.
- Lần đầu gộp làm đổi local → có file `sessions-before-sync-*.json`.
- Key: nhận về ghi đúng `keys\<id>_<tên>`, `KeyFilePath` trỏ đúng; `keyHash` không đổi → không ghi lại; tắt "file key" → VM nhận về đánh dấu ⚠ thiếu key.
- Mật khẩu nhận về được mã hóa DPAPI (không có chữ thường trong `sessions.json`).

**Tương thích:**
- File kho do engine tạo **Import được bằng `SessionImporter` hiện tại** (nhập mật khẩu đồng bộ) → đủ VM, đủ mật khẩu.
- Nội dung file kho **không** chứa mật khẩu/passphrase dạng chữ thường.
- `sessions.json` có `UpdatedAt`/`Deleted` vẫn được `SessionStore` của commit trước đọc đúng (test bằng cách deserialize với model cũ giữ trong test).

**Test Export/Import hiện có** phải qua nguyên sau khi tách `SntermCrypto`.

---

## 10. README (bổ sung)

Mục "Đồng bộ danh sách VM giữa nhiều máy": cách bật (2 loại kho), cách thêm máy thứ hai, cảnh báo bảo mật, xử lý sự cố (sai mật khẩu, "conflicted copy", lệch giờ, host key chưa tin cậy với SFTP), độ trễ phụ thuộc ổ đám mây, cách ngắt và xóa kho.

---

## 11. Các giai đoạn triển khai

### Giai đoạn S0 — Mô hình dữ liệu tương thích ngược
- `SessionInfo.UpdatedAt`, `SessionTombstone`, `SessionFileEnvelope.Deleted`, `SyncSettings` trong `AppSettings`, `SessionStore.Changed` + `Save(raiseChanged)`.
- Đặt `UpdatedAt` ở mọi điểm sửa nội dung (mục 3.1); xóa → tombstone.
- Test: file cũ không có `UpdatedAt` → = `CreatedAt`; ghi/đọc lại giữ `Deleted`; `LastConnectedAt` đổi không làm đổi `UpdatedAt`.

**Hoàn thành khi:** test qua; **[Tay]** dùng app như thường, mở `sessions.json` thấy `UpdatedAt`; xóa 1 VM thấy `Deleted` có 1 mục.

### Giai đoạn S1 — Mã hóa dùng chung, kho, gộp
- Tách `SntermCrypto` từ Exporter/Importer (test cũ qua nguyên).
- `ExportFile` thêm `sync`, `updatedAt`, `keyHash`; `SyncVault` đóng/mở kho; `SyncMerger`.
- Toàn bộ test `SyncMerger` và test tương thích (mục 9).

**Hoàn thành khi:** `dotnet test` qua, kể cả test Export/Import cũ.

### Giai đoạn S2 — Engine + backend Thư mục + lịch chạy
- `FolderSyncBackend`, `SyncEngine`, `SyncScheduler`, `SyncLog`, trạng thái qua sự kiện.
- Test engine với backend giả; test `FolderSyncBackend` trên thư mục tạm (ghi nguyên tử, retry khi file bị khóa, conflict copy).
- Tạm thời bật bằng cách sửa `settings.json` tay (chưa có UI).

**Hoàn thành khi:** test qua; **[Tay]** hai tài khoản Windows (hoặc `SNTERM_DATA_DIR` khác nhau — thêm biến này vào `AppPaths`, chỉ dùng kiểm thử) trỏ cùng thư mục: thêm VM bên này, bên kia có sau ≤ 20 s (debounce 10 s + chu kỳ); xóa cũng vậy.

### Giai đoạn S3 — Giao diện
- Nút ⟳, ô trạng thái status bar, `SyncSetupDialog` 3 bước, `SyncPreviewDialog`, mục Đồng bộ trong `SettingsDialog`, hộp nhập lại mật khẩu, chuỗi vi/en, icon xoay/đỏ.

**Hoàn thành khi:** **[Tay]**
- Máy A bật đồng bộ qua OneDrive, đặt mật khẩu; máy B bật cùng thư mục, nhập mật khẩu → xem trước đúng số VM → đủ VM, nhấp đúp vào thẳng (chỉ hỏi host key).
- Rút mạng ở A, sửa VM, cắm lại → tự đồng bộ, không hộp thoại nào, status bar đổi đúng.
- Đổi mật khẩu đồng bộ ở A → B báo "Cần mật khẩu đồng bộ", nhập đúng → chạy tiếp.
- Hủy ở bước xem trước → không đổi gì.
- Bố cục cửa sổ chính không khác v1 ngoài nút ⟳ và ô trạng thái.

### Giai đoạn S4 — Backend SFTP tới VM
- `SftpSyncBackend`, chọn VM trong `SyncSetupDialog`, `chmod 700/600`, host key qua `KnownHostsStore`.
- Test với container/WSL `openssh-server`.

**Hoàn thành khi:** **[Tay]** hai máy đồng bộ qua một VM; `ls -la ~/.snterm` thấy `drwx------` và `-rw-------`; VM chưa tin cậy host key → trạng thái hướng dẫn, mở VM một lần xong thì đồng bộ chạy.

### Giai đoạn S5 — Hoàn thiện
- Đồng bộ trước khi đóng app (≤ 5 s), dọn tombstone, cảnh báo lệch giờ, "Ngắt đồng bộ", "Xóa file kho", README mục 10, cập nhật `AppVersion` → `1.1.0`.
- **[Tay]** lấy file kho trên OneDrive → Import bằng bản **1.0.0** (bản không có đồng bộ) → nhập mật khẩu đồng bộ → đủ VM.

---

## 12. Khi chuyển sang v2 (Rust) — ánh xạ

| v1 C# (Phần 2) | v2 Rust (`crates/core/src/sync/`) |
|---|---|
| `ISyncBackend`, `FolderSyncBackend`, `SftpSyncBackend` | `backend/mod.rs` (`trait SyncBackend`), `backend/folder.rs`, `backend/sftp.rs` (dùng `core/ssh`) |
| `SntermCrypto`, `SyncVault` | `crypto/snterm_file.rs` (`encrypt_vault` / `decrypt_vault`), `sync/vault.rs` |
| `SyncMerger` | `sync/merge.rs` (hàm thuần, chuyển nguyên test) |
| `SyncEngine`, `SyncScheduler`, `SyncLog` | `sync/engine.rs` (tokio `Mutex`, `tokio::time`), log qua `tracing` |
| `SyncSetupDialog`, `SyncPreviewDialog`, mục Cài đặt | A: `components/dialogs/SyncSetup.svelte`, `SyncPreview.svelte`; B: `widgets/dialogs.rs` |
| Fixture kho `.vault` tạo bởi v1 | `tests/fixtures/sync/*.vault` — v2 phải đọc/ghi được **cùng kho** với v1 trong thời gian hai bản cùng tồn tại |

Yêu cầu bắt buộc khi chuyển: **định dạng kho, quy tắc gộp, tên trường JSON không đổi**, để một người dùng có máy chạy v1 và máy chạy v2 vẫn đồng bộ với nhau.

---

## 13. Rủi ro

| Rủi ro | Mức | Xử lý |
|---|---|---|
| Ổ đám mây tạo conflicted copy / đồng bộ trễ | Trung bình | Gộp file phụ (4.3), đọc meta lại trước khi ghi, README giải thích độ trễ |
| Lệch giờ giữa máy → chọn nhầm bản thắng | Thấp | Cảnh báo > 5 phút, quy tắc hòa ổn định, sao lưu trước lần gộp đầu, nhật ký để khôi phục |
| Kho lộ + mật khẩu yếu | Trung bình | ≥ 8 ký tự, thanh độ mạnh, cảnh báo trong hộp thoại và README |
| Quên mật khẩu đồng bộ | Trung bình | Không khôi phục được (như Export); hướng dẫn "Ngắt đồng bộ → Xóa kho → bật lại từ máy còn dữ liệu" |
| SFTP kho trên VM bị tắt | Thấp | Trạng thái ngoại tuyến, thử lại; dữ liệu cục bộ vẫn đầy đủ |
| Quy tắc "cùng host+port+user+tên" gộp nhầm 2 VM cố ý giống nhau | Thấp | Chỉ gộp khi **cả 4** trùng; người dùng đổi tên một VM là tách được |
| Tách `SntermCrypto` làm hỏng Export/Import | Thấp | Test cũ phải qua nguyên; fixture `.snterm` thật |
