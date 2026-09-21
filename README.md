# Ghita Download (Rust)

Phần mềm tải và chuyển đổi **âm thanh và video** chất lượng cao viết bằng ngôn ngữ **Rust**. Hỗ trợ tải trích xuất âm thanh từ **YouTube**, **Spotify**, **Suno AI**, **SoundCloud**, **Bandcamp** sang định dạng **MP3, FLAC (Lossless), AAC, WAV, luồng gốc (M4A/Opus)** và tải trọn vẹn **Video MP4** từ YouTube, TikTok, Facebook, Threads với các tùy chọn chất lượng tối ưu.

Ghita Download được thiết kế cho sự ổn định tuyệt đối: hỗ trợ tải song song đa luồng, tự động kiểm chứng thời lượng bài hát, tự động phát hiện & hỗ trợ cài đặt FFmpeg 1-click, tự cập nhật yt-dlp, hoạt động linh hoạt qua giao diện tương tác hoặc chế độ dòng lệnh (headless).

---

## Tính Năng Nổi Bật

- **Định dạng âm thanh đa dạng & Chuẩn phòng thu:**
  - **Lossless FLAC:** 24-bit 96kHz (Studio Master Hi-Res), 24-bit 48kHz (High-Res), 16-bit 44.1kHz (CD Quality Lossless).
  - **MP3 & AAC (.m4a):** Bitrate tùy chọn từ 64 kbps đến 320 kbps (hỗ trợ chuẩn iTunes Plus 256k).
  - **WAV & Audio Gốc:** WAV PCM nguyên bản (lên tới 24-bit 48kHz) hoặc giữ nguyên luồng M4A/Opus gốc không suy hao.
  - **Video MP4:** Tải video chất lượng cao, tùy chọn giới hạn trần độ phân giải 1080p, 720p, 480p hoặc tốt nhất (best).

- **Đa nền tảng nhạc số & Mạng xã hội:**
  - **YouTube & YouTube Music:** Tải âm thanh hoặc video từ clip thông thường, Shorts, bài hát và playlist chính thức.
  - **Spotify, SoundCloud & Bandcamp:** Tải track đơn hoặc trọn bộ Album / Playlist; tự động nhúng ảnh bìa HD và siêu dữ liệu chính thức.
  - **Suno AI (Nhạc AI):** Tải trực tiếp bài hát AI nguyên bản chất lượng cao kèm ảnh bìa tác phẩm do AI tạo ra.
  - **Mạng xã hội & Video ngắn:** Tải video và trích xuất âm thanh từ TikTok, Facebook Video và Threads.

- **Tự động hóa & Khả năng tương thích cao:**
  - **1-Click FFmpeg Setup:** Tự động phát hiện và hỗ trợ cài đặt công cụ giải mã FFmpeg cho máy tính mới qua winget hoặc tải bản static nhúng sẵn.
  - **Tự cập nhật yt-dlp:** Cập nhật công cụ tải lên phiên bản mới nhất từ GitHub Releases chính thức trực tiếp trong menu hoặc qua dòng lệnh.
  - **Tùy chọn dấu tiếng Việt:** Mặc định khử dấu an toàn (`ASCII`) cho màn hình xe hơi/USB cổ điển, hoặc bật giữ nguyên Unicode tiếng Việt (`Nguyễn Văn A - Bài Hát.mp3`).
  - **Đánh số thứ tự track:** Tự động thêm tiền tố thứ tự (`01. Tên bài.mp3`) và nhúng thẻ tag ID3/Vorbis track index cho Album/Playlist.

- **Độ tin cậy & Trải nghiệm tương tác:**
  - **Tải song song & Kiểm chứng:** Tải nhiều luồng đồng thời; tự động đối chiếu thời lượng (sai số ±10s) chống tải nhầm bản cover/live/remix.
  - **Hàng đợi thử lại:** Tự động lưu bài lỗi vào `failed_tasks.json` để thử lại nhanh bất kỳ lúc nào.
  - **Dashboard thông minh:** Tự động ghi nhớ cấu hình tải; hỗ trợ nhập dán nhiều liên kết cùng lúc hoặc chạy ngầm (headless) cho kịch bản tự động hóa.

---

## Hướng Dẫn Cài Đặt & Thiết Lập (1-Click Installer)

Ghita Download cung cấp **tệp cài đặt duy nhất nằm trong thư mục `Release/` (`Release/ghitadownload_0.0.2.exe`)** tự động giải nén ứng dụng, nhúng sẵn công cụ phụ trợ, kiểm tra FFmpeg và cấu hình biến môi trường `PATH` để bạn có thể gọi từ bất kỳ thư mục nào trên máy tính.

### Cách 1: Cài đặt bằng tệp trong `Release/` (Khuyên dùng)
1. Mở thư mục **`Release/`**, nhấp đúp chuột vào tệp **`ghitadownload_0.0.2.exe`**.
2. Chương trình sẽ hiển thị giao diện cài đặt tương tác:
   - Thư mục cài đặt mặc định: `%LOCALAPPDATA%\GhitaDownload`
   - Nhấn **[ENTER]** để xác nhận cài đặt ngay (hoặc dán đường dẫn thư mục tùy chỉnh nếu muốn).
3. Trình cài đặt sẽ tự động:
   - Cài đặt `ghitadownload.exe` và alias gọi nhanh `ghita.exe`.
   - Cài đặt công cụ hỗ trợ `bin/yt-dlp.exe`.
   - Kiểm tra và tự động hỗ trợ cài đặt công cụ giải mã âm thanh `bin/ffmpeg.exe` nếu máy chưa có.
   - **Tự động thêm đường dẫn vào biến môi trường PATH** của người dùng (không cần quyền Administrator).
4. Nhấn **[ENTER]** để hoàn tất.

### Cách 2: Tự biên dịch từ mã nguồn (Dành cho lập trình viên)
1. Đảm bảo máy đã cài đặt [Rust & Cargo](https://rustup.rs).
2. Nhấp đúp vào tệp **`build_installer.bat`** (hoặc chạy lệnh):
   ```powershell
   cargo build --release --bin ghitadownload
   cargo build --release --bin ghitadownload-setup
   if not exist Release mkdir Release
   copy target\release\ghitadownload-setup.exe Release\ghitadownload_0.0.2.exe
   ```
3. Chạy file `Release/ghitadownload_0.0.2.exe` vừa tạo để cài đặt vào hệ thống.

---

## Cách Sử Dụng Từ Bất Kỳ Thư Mục Nào

Sau khi đã cài đặt, bạn có thể tải nhạc và video trực tiếp vào bất kỳ thư mục nào:

1. Mở thư mục bạn muốn lưu tệp trong **File Explorer** (ví dụ: `D:\NhacCuaToi` hoặc `Desktop`).
2. Nhấp chuột phải chọn **"Open in Terminal"** (hoặc gõ `powershell` / `cmd` lên thanh địa chỉ thư mục rồi nhấn Enter).
3. Gõ lệnh:
   ```powershell
   ghitadownload
   ```
   *hoặc gõ nhanh:*
   ```powershell
   ghita
   ```
4. Giao diện điều khiển Ghita sẽ mở ra ngay lập tức! Các bài hát và video tải về sẽ được lưu ngay tại chính thư mục bạn vừa mở.

### Các thao tác chính trong giao diện Terminal

1. **Menu điều khiển chính (Dashboard):**
   - `[1] 🚀 Bắt đầu nhập liên kết & Tải ngay`: Sử dụng cấu hình đang lưu, vào thẳng màn hình nhập link.
   - `[2] ⚙ Thay đổi cài đặt`: Tùy chỉnh thư mục lưu, định dạng (MP3/FLAC/AAC/WAV/Gốc/Video), chất lượng bitrate, độ phân giải video và tùy chọn giữ/khử dấu tiếng Việt.
   - `[3] 🔁 Thử lại các bài lỗi gần nhất`: Nạp `failed_tasks.json` trong thư mục lưu và tải lại đúng các bài đã lỗi.
   - `[4] 📂 Mở thư mục lưu nhạc`: Mở nhanh thư mục tải trong Windows Explorer.
   - `[5] 🔄 Cập nhật công cụ yt-dlp`: Tự động tải bản cập nhật mới nhất của yt-dlp từ GitHub Release.
   - `[6] ❌ Thoát`: Đóng ứng dụng.

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

# Tải định dạng Lossless FLAC chuẩn phòng thu 24-bit 96kHz và giữ nguyên dấu tiếng Việt
ghitadownload --link "https://open.spotify.com/album/..." --format flac --quality 24bit96k --keep-accents --output "D:\NhacLossless"

# Tải AAC iTunes Plus 256k
ghitadownload --link "https://open.spotify.com/track/..." --format aac --quality 256k --output "D:\Nhac"

# Tải danh sách liên kết từ một tệp văn bản (mỗi dòng một link) sang WAV CD Lossless
ghitadownload --file links.txt --format wav --quality 16bit44k --output "D:\Nhac"

# Giữ nguyên âm thanh gốc (M4A/Opus) hoặc tải video MP4 kèm trần độ phân giải
ghitadownload --link "https://soundcloud.com/..." --format original --output "D:\Nhac"
ghitadownload --link "https://youtu.be/..." --format video --resolution 1080 --output "D:\Video"

# Cập nhật yt-dlp lên phiên bản mới nhất từ GitHub
ghitadownload --update-ytdlp

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
| `--output <DIR>` | Thư mục lưu bài hát hoặc video. |
| `--format <FMT>` | `mp3` \| `flac` \| `aac` \| `wav` \| `original` (M4A/Opus) \| `video` (MP4). |
| `--quality <Q>` | Mức chất lượng: `320k`/`256k`/`192k`/`128k`/`96k`/`64k` (MP3, AAC); `24bit96k`/`24bit48k`/`16bit44k` (FLAC); `24bit48k`/`16bit44k`/`16bit22k`/`8bit11k` (WAV). |
| `--resolution <RES>` | Trần độ phân giải video: `1080` \| `720` \| `480` \| `best`. |
| `--keep-accents` | Giữ nguyên dấu tiếng Việt Unicode thay vì chuyển đổi sang tên tệp ASCII không dấu an toàn. |
| `--concurrency <N>` | Số bài tải song song (mặc định 3). |
| `--retry-failed` | Đọc `failed_tasks.json` trong `--output` và tải lại các bài lỗi. |
| `--update-ytdlp` (alias `--update-tools`) | Tải và cập nhật bản `yt-dlp` mới nhất từ GitHub Releases. |

Ở chế độ headless, ứng dụng trả **mã thoát `0`** khi tất cả bài thành công và **`1`** khi còn bài lỗi (đã được ghi vào `failed_tasks.json` để thử lại).

---

## Gỡ Cài Đặt (Uninstall)

Khi không còn nhu cầu sử dụng, bạn có thể gỡ cài đặt sạch sẽ khỏi hệ thống:
- **Cách 1:** Mở thư mục cài đặt `%LOCALAPPDATA%\GhitaDownload` và nhấp đúp vào **`uninstall.bat`**.
- **Cách 2:** Chạy lệnh từ terminal:
  ```powershell
  Release\ghitadownload_0.0.2.exe --uninstall
  ```
Trình gỡ cài đặt sẽ tự động dọn dẹp thư mục và xóa đường dẫn khỏi biến môi trường `PATH`.

---

## Kiểm Thử & Phát Triển

Chạy toàn bộ test suite để kiểm tra tính năng và tính toàn vẹn:
```powershell
cargo test
```
