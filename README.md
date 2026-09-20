# Ghita Downloader (Rust)

Phần mềm tải nhạc chất lượng cao viết bằng ngôn ngữ **Rust**, hỗ trợ tải từ **YouTube**, **YouTube Music**, **Spotify**, **Suno AI**, **SoundCloud**, **Bandcamp**, **TikTok**, **Facebook**, **Threads** sang định dạng **.mp3**, **.wav**, giữ nguyên **gốc (M4A/Opus)** hoặc **video (.mp4)** với đầy đủ các mức chất lượng tùy chỉnh. Ứng dụng tải **song song nhiều bài**, **tự kiểm chứng** tệp tải về so với thời lượng chuẩn, **tự động thử lại** các bài lỗi và có cả **chế độ dòng lệnh nhanh (headless)** không cần giao diện.

---

## Tính Năng Nổi Bật

- **Hỗ trợ đa nền tảng nhạc số & mạng xã hội:**
  - **YouTube & YouTube Music:** Video thông thường, YouTube Shorts, các bài hát và playlist chính thức.
  - **Spotify:** Track đơn lẻ, trích xuất toàn bộ Album hoặc Playlist (tự động lấy metadata và ảnh bìa HD/4K chính thức).
  - **Suno AI:** Tải trực tiếp bài hát và ảnh bìa AI chất lượng cao.
  - **SoundCloud & Bandcamp:** Tải đường dẫn bài/album trực tiếp hoặc qua liên kết chia sẻ.
  - **Mạng xã hội:** TikTok, Facebook Video, Threads.
- **Nhiều định dạng đầu ra:**
  - **MP3 / WAV:** Chuyển mã với dải bitrate tùy chọn (xem bên dưới).
  - **ORIGINAL (M4A/Opus):** Giữ nguyên luồng âm thanh gốc, không chuyển mã (nhanh, không suy hao).
  - **VIDEO (.mp4):** Giữ nguyên hình ảnh, chọn trần độ phân giải 1080p/720p/480p/cao nhất, không chuyển mã.
- **Tải hàng loạt song song:**
  - Mặc định tải đồng thời 3 bài (đổi được bằng `--concurrency` ở chế độ headless), mỗi bài một thanh tiến độ riêng.
  - Thanh tiến độ hiển thị **% và dung lượng thật** lấy trực tiếp từ luồng xuất của yt-dlp.
- **Kiểm chứng tự động sau khi tải:**
  - Đối chiếu thời lượng tệp tải về với thời lượng chuẩn của Spotify/YouTube (ngưỡng lệch ±10 giây), chống tải nhầm cover/live/remix khi tìm kiếm dự phòng.
  - Chặn các tệp hỏng/quá nhỏ (dưới 10 KB hoặc dưới 3 giây) và tự chuyển sang phương án tải dự phòng kế tiếp.
- **Hàng đợi thử lại (failed_tasks.json):**
  - Các bài lỗi trong một đợt được lưu lại kèm cấu hình đã dùng; có thể thử lại đúng các bài đó từ menu hoặc bằng lệnh `--retry-failed`.
- **Bộ nhập liệu tương tác thông minh (Multi-Link Interactive Input):**
  - Nhập liên kết #1 ➔ Nhấn **[Mũi tên XUỐNG ↓]** để nhập tiếp liên kết #2, #3, #4, #5...
  - Nhấn **[Mũi tên LÊN ↑]** để quay lại chỉnh sửa liên kết trước.
  - Hỗ trợ dán (Ctrl+V) hàng loạt liên kết cùng lúc (tự động chia dòng).
  - Nhấn **[ENTER]** khi hoàn tất để bắt đầu tải.
- **Dashboard điều khiển nhanh (Quick Start Menu):**
  - Tự động ghi nhớ cấu hình (thư mục, định dạng, bitrate, độ phân giải) từ lần chạy trước.
  - Bấm chọn tải ngay lập tức mà không phải trả lời lại 3-4 câu hỏi cài đặt mỗi lần.
- **Cơ chế tải siêu ổn định (100% Reliability):**
  - Smart Search Fallback: Tự động thử lại với nhiều biến thể từ khóa và top 3 kết quả khi tìm kiếm bài hát Spotify trên YouTube.
  - Chống chặn bot yt-dlp và ngắt kết nối (`--extractor-args`, `--retries 5`, `--socket-timeout 30`).
  - Chuẩn hóa ảnh bìa ID3 tương thích 100% với Windows File Explorer & Media Player.
- **Đầy đủ dải chất lượng âm thanh:**
  - **MP3:** 320 kbps (Cực cao/Studio), 256 kbps (Chuẩn YouTube Music), 192 kbps (Chuẩn), 128 kbps (Trung bình), 96 kbps (Thấp), 64 kbps (Thấp nhất).
  - **WAV:** 24-bit PCM 48kHz (Master Studio Lossless), 16-bit PCM 44.1kHz (CD Quality Lossless), 16-bit 22.05kHz, 8-bit 11.025kHz (Lo-Fi).
- **Chế độ dòng lệnh nhanh (Headless):** Tải ngay bằng tham số mà không mở giao diện, phù hợp tự động hóa/tập lệnh.
- **Tùy chọn thư mục lưu (Custom Output Folder):**
  - Nhập đường dẫn bất kỳ trên máy tính để lưu nhạc.
  - Có tùy chọn mở ngay thư mục lưu trong Windows File Explorer từ menu.

---

## Hướng Dẫn Cài Đặt & Thiết Lập (1-Click Installer)

Ghita Downloader cung cấp **tệp cài đặt duy nhất nằm trong thư mục `Release/` (`Release/ghitadownload_0.0.1.exe`)** tự động giải nén ứng dụng, nhúng sẵn công cụ phụ trợ và cấu hình biến môi trường `PATH` để bạn có thể gọi từ bất kỳ thư mục nào trên máy tính.

### Cách 1: Cài đặt bằng tệp trong `Release/` (Khuyên dùng)
1. Mở thư mục **`Release/`**, nhấp đúp chuột vào tệp **`ghitadownload_0.0.1.exe`**.
2. Chương trình sẽ hiển thị giao diện cài đặt tương tác:
   - Thư mục cài đặt mặc định: `%LOCALAPPDATA%\GhitaDownload`
   - Nhấn **[ENTER]** để xác nhận cài đặt ngay (hoặc dán đường dẫn thư mục tùy chỉnh nếu muốn).
3. Trình cài đặt sẽ tự động:
   - Cài đặt `ghitadownload.exe` và alias gọi nhanh `ghita.exe`.
   - Cài đặt công cụ hỗ trợ `bin/yt-dlp.exe`.
   - **Tự động thêm đường dẫn vào biến môi trường PATH** của người dùng (không cần quyền Administrator).
4. Nhấn **[ENTER]** để hoàn tất.

### Cách 2: Tự biên dịch từ mã nguồn (Dành cho lập trình viên)
1. Đảm bảo máy đã cài đặt [Rust & Cargo](https://rustup.rs).
2. Nhấp đúp vào tệp **`build_installer.bat`** (hoặc chạy lệnh):
   ```powershell
   cargo build --release --bin ghitadownload
   cargo build --release --bin ghitadownload-setup
   if not exist Release mkdir Release
   copy target\release\ghitadownload-setup.exe Release\ghitadownload_0.0.1.exe
   ```
3. Chạy file `Release/ghitadownload_0.0.1.exe` vừa tạo để cài đặt vào hệ thống.

---

## Cách Sử Dụng Từ Bất Kỳ Thư Mục Nào

Sau khi đã cài đặt, bạn có thể tải nhạc trực tiếp vào bất kỳ thư mục nào bạn muốn:

1. Mở thư mục bạn muốn lưu nhạc trong **File Explorer** (ví dụ: `D:\NhacCuaToi` hoặc `Desktop`).
2. Nhấp chuột phải chọn **"Open in Terminal"** (hoặc gõ `powershell` / `cmd` lên thanh địa chỉ thư mục rồi nhấn Enter).
3. Gõ lệnh:
   ```powershell
   ghitadownload
   ```
   *hoặc gõ nhanh:*
   ```powershell
   ghita
   ```
4. Giao diện điều khiển Ghita sẽ mở ra ngay lập tức! Các bài hát tải về sẽ được lưu ngay tại chính thư mục bạn vừa mở.

### Các thao tác chính trong giao diện Terminal:
1. **Menu điều khiển chính (Dashboard):**
   - `[1] 🚀 Bắt đầu nhập liên kết & Tải ngay`: Sử dụng cấu hình đang lưu, vào thẳng màn hình nhập link.
   - `[2] ⚙ Thay đổi cài đặt`: Tùy chỉnh thư mục lưu, định dạng MP3/WAV/Gốc(M4A-Opus)/Video(MP4), mức chất lượng bitrate hoặc độ phân giải video.
   - `[3] 🔁 Thử lại các bài lỗi gần nhất`: Nạp `failed_tasks.json` trong thư mục lưu và tải lại đúng các bài đã lỗi.
   - `[4] 📂 Mở thư mục lưu nhạc`: Mở nhanh thư mục tải trong Windows Explorer.
   - `[5] ❌ Thoát`: Đóng ứng dụng.

2. **Màn hình nhập đa liên kết:**
   - Gõ hoặc dán link 1 ➔ Nhấn **[↓ Mũi tên Xuống]** ➔ Xuất hiện `[Liên kết #2] > `
   - Tiếp tục gõ link 2 ➔ Nhấn **[↓]** ➔ `[Liên kết #3] > ` ...
   - Nhấn **[↑ Mũi tên Lên]** nếu muốn sửa lại link phía trên.
   - Nhấn **[ENTER]** khi hoàn tất để ứng dụng giải nén và tải song song từng bài hát.
   - Nhấn **[ESC]** nếu muốn hủy quay lại Menu chính.

---

## Chế Độ Dòng Lệnh Nhanh (Headless)

Ngoài giao diện tương tác, bạn có thể tải ngay bằng tham số mà không cần mở menu (phù hợp tự động hóa, tập lệnh). Chỉ cần truyền liên kết là ứng dụng tự chạy ở chế độ headless.

```powershell
# Tải một hoặc nhiều liên kết sang MP3 320k vào thư mục chỉ định
ghitadownload --link "https://open.spotify.com/track/..." --link "https://youtu.be/..." --format mp3 --quality 320k --output "D:\Nhac"

# Tải danh sách liên kết từ một tệp văn bản (mỗi dòng một link)
ghitadownload --file links.txt --format wav --quality 16bit44k --output "D:\Nhac"

# Giữ nguyên âm thanh gốc (M4A/Opus) hoặc tải video MP4 kèm trần độ phân giải
ghitadownload --link "https://soundcloud.com/..." --format original --output "D:\Nhac"
ghitadownload --link "https://youtu.be/..." --format video --resolution 1080 --output "D:\Video"

# Đổi số bài tải song song (mặc định 3)
ghitadownload --link "https://youtu.be/..." --format mp3 --concurrency 5 --output "D:\Nhac"

# Thử lại các bài lỗi đã lưu trong failed_tasks.json của thư mục xuất
ghitadownload --retry-failed --output "D:\Nhac"
```

**Tham số chính:**

| Tham số | Ý nghĩa |
| --- | --- |
| `--link <URL>` (alias `--links`) | Một hoặc nhiều liên kết nguồn (lặp lại nhiều lần được). |
| `--file <FILE>` | Tệp văn bản chứa danh sách liên kết (mỗi dòng một link). |
| `--output <DIR>` | Thư mục lưu bài hát. |
| `--format <FMT>` | `mp3` \| `wav` \| `original` (M4A/Opus) \| `video` (MP4). |
| `--quality <Q>` | `320k`/`256k`/`192k`/`128k`/`96k`/`64k` hoặc `24bit48k`/`16bit44k`/`16bit22k`/`8bit11k`. |
| `--resolution <RES>` | Trần độ phân giải video: `1080` \| `720` \| `480` \| `best`. |
| `--concurrency <N>` | Số bài tải song song (mặc định 3). |
| `--retry-failed` | Đọc `failed_tasks.json` trong `--output` và tải lại các bài lỗi. |

Ở chế độ headless, ứng dụng trả **mã thoát `0`** khi tất cả bài thành công và **`1`** khi còn bài lỗi (đã được ghi vào `failed_tasks.json` để thử lại).

---

## Gỡ Cài Đặt (Uninstall)

Khi không còn nhu cầu sử dụng, bạn có thể gỡ cài đặt sạch sẽ khỏi hệ thống:
- **Cách 1:** Mở thư mục cài đặt `%LOCALAPPDATA%\GhitaDownload` và nhấp đúp vào **`uninstall.bat`**.
- **Cách 2:** Chạy lệnh từ terminal:
  ```powershell
  Release\ghitadownload_0.0.1.exe --uninstall
  ```
Trình gỡ cài đặt sẽ tự động dọn dẹp thư mục và xóa đường dẫn khỏi biến môi trường `PATH`.

---

## Kiểm Thử & Phát Triển

Chạy toàn bộ test suite để kiểm tra tính năng và tính toàn vẹn:
```powershell
cargo test
```
