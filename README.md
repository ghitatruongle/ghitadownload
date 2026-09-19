# Ghita Downloader (Rust)

Phần mềm tải nhạc chất lượng cao viết bằng ngôn ngữ **Rust**, hỗ trợ tải từ **YouTube**, **YouTube Music**, **Spotify**, **Suno AI**, **TikTok**, **Facebook**, **Threads** sang định dạng **.mp3** hoặc **.wav** với đầy đủ các mức chất lượng tùy chỉnh (từ cao nhất đến thấp nhất), hỗ trợ tải hàng loạt và chọn thư mục lưu bài hát.

---

## Tính Năng Nổi Bật

- **Hỗ trợ đa nền tảng nhạc số & mạng xã hội:**
  - **YouTube:** Video thông thường, YouTube Shorts.
  - **YouTube Music (`music.youtube.com`):** Các bài hát và playlist chính thức.
  - **Spotify:** Track đơn lẻ, trích xuất toàn bộ Album hoặc Playlist (tự động lấy metadata và ảnh bìa HD/4K chính thức).
  - **Suno AI:** Tải trực tiếp bài hát và ảnh bìa AI chất lượng cao.
  - **Mạng xã hội:** TikTok, Facebook Video, Threads.
- **Bộ nhập liệu tương tác thông minh (Multi-Link Interactive Input):**
  - Nhập liên kết #1 ➔ Nhấn **[Mũi tên XUỐNG ↓]** để nhập tiếp liên kết #2, #3, #4, #5...
  - Nhấn **[Mũi tên LÊN ↑]** để quay lại chỉnh sửa liên kết trước.
  - Hỗ trợ dán (Ctrl+V) hàng loạt liên kết cùng lúc (tự động chia dòng).
  - Nhấn **[ENTER]** khi hoàn tất để bắt đầu tải tuần tự.
- **Dashboard điều khiển nhanh (Quick Start Menu):**
  - Tự động ghi nhớ cấu hình (thư mục, định dạng, bitrate) từ lần chạy trước.
  - Bấm chọn tải ngay lập tức mà không phải trả lời lại 3-4 câu hỏi cài đặt mỗi lần.
- **Cơ chế tải siêu ổn định (100% Reliability):**
  - Smart Search Fallback: Tự động thử lại với nhiều biến thể từ khóa và top 3 kết quả khi tìm kiếm bài hát Spotify trên YouTube.
  - Chống chặn bot yt-dlp và ngắt kết nối (`--extractor-args`, `--retries 5`, `--socket-timeout 30`).
  - Chuẩn hóa ảnh bìa ID3 tương thích 100% với Windows File Explorer & Media Player.
- **Đầy đủ dải chất lượng âm thanh:**
  - **MP3:** 320 kbps (Cực cao/Studio), 256 kbps (Chuẩn YouTube Music), 192 kbps (Chuẩn), 128 kbps (Trung bình), 96 kbps (Thấp), 64 kbps (Thấp nhất).
  - **WAV:** 24-bit PCM 48kHz (Master Studio Lossless), 16-bit PCM 44.1kHz (CD Quality Lossless), 16-bit 22.05kHz, 8-bit 11.025kHz (Lo-Fi).
- **Tùy chọn thư mục lưu (Custom Output Folder):**
  - Nhập đường dẫn bất kỳ trên máy tính để lưu nhạc.
  - Có tùy chọn mở ngay thư mục lưu trong Windows File Explorer từ menu.

---

## Hướng Dẫn Cài Đặt & Thiết Lập (1-Click Installer)

Ghita Downloader cung cấp **tệp cài đặt duy nhất nằm trong thư mục `Release/` (`Release/ghitadownload_0.0.0.exe`)** tự động giải nén ứng dụng, nhúng sẵn công cụ phụ trợ và cấu hình biến môi trường `PATH` để bạn có thể gọi từ bất kỳ thư mục nào trên máy tính.

### Cách 1: Cài đặt bằng tệp trong `Release/` (Khuyên dùng)
1. Mở thư mục **`Release/`**, nhấp đúp chuột vào tệp **`ghitadownload_0.0.0.exe`**.
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
   copy target\release\ghitadownload-setup.exe Release\ghitadownload_0.0.0.exe
   ```
3. Chạy file `Release/ghitadownload_0.0.0.exe` vừa tạo để cài đặt vào hệ thống.

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
   - `[2] ⚙ Thay đổi cài đặt`: Tùy chỉnh thư mục lưu, định dạng MP3/WAV hoặc mức chất lượng bitrate.
   - `[3] 📂 Mở thư mục lưu nhạc`: Mở nhanh thư mục tải trong Windows Explorer.
   - `[4] ❌ Thoát`: Đóng ứng dụng.

2. **Màn hình nhập đa liên kết:**
   - Gõ hoặc dán link 1 ➔ Nhấn **[↓ Mũi tên Xuống]** ➔ Xuất hiện `[Liên kết #2] > `
   - Tiếp tục gõ link 2 ➔ Nhấn **[↓]** ➔ `[Liên kết #3] > ` ...
   - Nhấn **[↑ Mũi tên Lên]** nếu muốn sửa lại link phía trên.
   - Nhấn **[ENTER]** khi hoàn tất để ứng dụng giải nén và tải tuần tự từng bài hát.
   - Nhấn **[ESC]** nếu muốn hủy quay lại Menu chính.

---

## Gỡ Cài Đặt (Uninstall)

Khi không còn nhu cầu sử dụng, bạn có thể gỡ cài đặt sạch sẽ khỏi hệ thống:
- **Cách 1:** Mở thư mục cài đặt `%LOCALAPPDATA%\GhitaDownload` và nhấp đúp vào **`uninstall.bat`**.
- **Cách 2:** Chạy lệnh từ terminal:
  ```powershell
  Release\ghitadownload_0.0.0.exe --uninstall
  ```
Trình gỡ cài đặt sẽ tự động dọn dẹp thư mục và xóa đường dẫn khỏi biến môi trường `PATH`.

---

## Kiểm Thử & Phát Triển

Chạy toàn bộ test suite để kiểm tra tính năng và tính toàn vẹn:
```powershell
cargo test
```
