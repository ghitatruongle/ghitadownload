# Changelog

## [0.0.4] - 2026-10-04

### Added
- Suno playlist pagination collecting every page of clips with deduplication by clip id.
- Ctrl+C cancellation that kills child yt-dlp/FFmpeg processes, marks unfinished tasks as failed, and still writes the retry queue.
- `--cookies <file>` and `--cookies-from-browser <browser>` flags forwarded to yt-dlp for login-gated sources.
- `--format opus` output with `192k`/`128k`/`96k` quality options.
- Cover art and metadata embedding for FLAC and M4A (AAC) outputs in addition to MP3.
- JSON manifest `ghita_manifest_<timestamp>.json` written after every batch listing per-task status and output paths.
- Stale `.ghita_temp_*` directory sweep at batch start.
- Backoff with jitter between every yt-dlp retry attempt, and a shared HTTP retry helper with exponential backoff for Spotify/Suno 429 and timeout responses.
- Shared `utils/fs::replace_file`, shared HTTP client in `utils/http`, yt-dlp argument constants in `core/ytdlp`, and a `build_task` helper replacing six duplicated task literals.
- Info line when a local `ghita_config.json` overrides the global configuration.
- CI hardening: `cargo audit` job, yt-dlp download cache, installer smoke check, CHANGELOG/tag cross-check, and Dependabot for cargo and GitHub Actions.

### Changed
- Replaced the nested `current_thread` runtime inside the Suno decrypt blocking task with a direct async call.
- JoinError mapping records the exact failed task index instead of the first unfinished slot.
- Removed the hard-coded Microsoft Store Python path from yt-dlp discovery; installed locations and `PATH` remain supported.

### Release artifact
- `Release/ghitadownload_0.0.4.exe`
- SHA-256: `397D03FF55F0188F80AD7BBE1FFA7C2288D008DDECFAA5F11C5783D5C00ADE26`

### Compatibility
- Existing CLI flags, format/quality names, exit codes, and `AppConfig`/`FailedQueue` JSON remain compatible. `--format opus` does not change defaults of existing formats.

## [0.0.3] - 2026-10-01

### Added
- GitHub Actions CI running format check, clippy with warnings as errors, and the locked test suite on pushes and pull requests targeting `main`.
- Tag-driven release workflow that validates the `v*` tag against the package version, downloads the pinned yt-dlp payload by SHA-256, builds and verifies the installer, and publishes it to GitHub Releases.
- Platform parser regression tests covering `youtu.be`, `/embed/` and `/v/` paths, YouTube Music playlists, Spotify URI album/playlist variants, uppercase and extended direct-media extensions, Suno alternate hosts, `fb.watch`, empty input, and playlist classification.
- Tagger regression tests covering ID3 field round-trip, cover-less tagging without network access, and missing-file rejection.
- Batch regression tests covering empty-queue execution and corrupt or wrong-shape retry queue JSON.

### Changed
- Split the 1,010-line batch module into `batch/mod.rs`, `batch/resolve.rs`, `batch/task.rs`, and `batch/execute.rs` without changing the public API or behavior.
- Added clippy with warnings as errors to the `build_installer.bat` release gate alongside format and test checks.
- Made `scripts/verify_release.ps1` version-agnostic; the embedded version evidence is still checked against the version read from Cargo.toml.
- The installer skips its final pause when `GHITA_CI` is set so automated environments can run it unattended.
- Promoted the release channel from `0.0.3-beta` to stable `0.0.3`.

### Fixed
- Depth-based qualities now coerce across WAV and FLAC: `--format flac --quality 24bit48k` / `16bit44k` and `--format wav --quality flac24bit48k` work as documented instead of being rejected with an incompatibility error.
- Suno failures now surface the real cause: when the Suno clip API returns a non-success status, the resulting error includes the actual HTTP status code and a region/IP or proxy hint instead of silently falling through to a misleading yt-dlp DRM error.
- yt-dlp discovery now tries the development repository payload (`<exe>/../../bin/yt-dlp.exe`) before previously installed copies, so a stale helper left by an older install (or a silent-exiting stub) can no longer break every download with an empty error; the official pinned executable is preferred in development runs and the installed copy remains first for installed layouts.
- Plain bitrate qualities now coerce to the matching variant of the chosen format: `--format opus --quality 128k` and `--format aac --quality 256k` work as documented instead of being rejected with an incompatibility error; only MP3 accepted plain bitrates before. Out-of-range combinations (e.g. `64k` for Opus, `320k` for FLAC) are still rejected, and the saved last quality is coerced the same way.
- Aligned the pinned yt-dlp SHA-256 with the official `2026.08.19` release artifact (`66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a`). The previous pin matched a non-official 108 KB payload while the manifest URL serves the official executable, so CI verification and `--update-ytdlp` would both reject the official file; the bundled payload is now the official executable and all three pins (app constant, manifest, and SHA file) agree.
- Hoisted the Suno UUID regex out of the per-download verification loop and replaced manual character comparisons flagged by clippy.
- Per-task and resolve-stage logs are now always printed when the output is piped or redirected (headless, CI, log files); previously indicatif dropped them on hidden draw targets, contradicting the headless logging guarantee.
- Downloading a silent, video-only source in an audio format (MP3/WAV/FLAC/AAC) now fails fast with a clear "no audio stream" reason during verification instead of an obscure FFmpeg "no stream" error during transcoding; `--format original` and `--format video` still save such sources.

### Release artifact
- `Release/ghitadownload_0.0.3.exe`
- SHA-256: `6A2376B46EF4C219F105FB2D98F2009EF02521762BDDDC78B3AACAA01DF0F341`
- Verified with `scripts/verify_dependencies.ps1` and `scripts/verify_release.ps1`.

### Compatibility
- Existing CLI flags, configuration JSON, audio/video format names, and installer arguments remain compatible.

## [0.0.3-beta] - 2026-09-24

### Added
- Windows beta installer with synchronized application and package versions.
- Focused CLI, configuration, and installer helper regression tests.
- Media signature and DRM/forbidden Suno payload regression tests.

### Release artifact
- `Release/ghitadownload_0.0.3-beta.exe`
- SHA-256: `2E7A7246E6F7A9C25DE916C26A4F1422EDBC7B864EEE33E2AC8BEF2888E1B9EA`
- Verified with `scripts/verify_dependencies.ps1` and `scripts/verify_release.ps1`.

### Fixed
- Reject non-media payloads before FFmpeg probe using container signatures (MP4/M4A/MP3/WAV/FLAC/OGG/WebM) and require a real audio/video stream instead of misreporting encrypted blobs as LRC.
- Resolve Suno tracks from `studio-api.prod.suno.com/api/clip/{id}`, collect `media_urls` safely, and fail fast on `audio_url` forbidden or DRM `encoding` (e.g. `1.0.0`) with a clear reason instead of downloading garbage.
- Carry Suno duration metadata into validation and skip the unusable yt-dlp Suno-page fallback.
- Classify direct media file URLs (mp3/m4a/wav/flac/ogg/opus/webm/mp4/...) as `DirectMedia` so CDN links are not wrapped as `ytsearch1:`.
- Increase yt-dlp retries and always print per-task verification logs in headless mode.
- Prefer absolute executable-relative and installed tool paths before validated `PATH` entries instead of trusting the current working directory or bare command names.
- Download yt-dlp and FFmpeg through unique temporary paths, validate size, Windows PE headers, version, and configured SHA-256 where available, and preserve the previous tool if replacement fails.
- Pin the bundled yt-dlp baseline to `2026.08.19` and `f0a2417a49d9dbbc17d76d2bf37192231eec4930e9d68c45c011788a4641b0b9`; require explicit URL and SHA-256 for alternate downloads and for FFmpeg auto-install.
- Validate headless argument combinations and format/quality/resolution/concurrency constraints before dependency or filesystem work.
- Make headless operation non-interactive and return nonzero for missing dependencies, including interactive startup failures.
- Surface configuration read/write errors and save configuration through an atomic temporary file with rollback protection.
- Use the saved/default output directory when headless `--output` is omitted; first-run defaults to the system music/downloads directory.
- Reject unknown installer arguments, persist custom install locations, make uninstall path-aware, and avoid recursive deletion.
- Check user `PATH` reads before writes and stop rather than overwriting `PATH` after a read failure.
- Read Suno metadata and current `media_urls` instead of relying on obsolete CDN URLs; media protected by Suno DRM is rejected rather than reported as a valid download.
- Build with `cargo build --locked` and verify a candidate artifact before replacing the public release artifact.
- Verify Windows PE headers plus embedded package-version and payload-name evidence. This is integrity evidence, not a publisher signature or a committed expected digest.

### Compatibility
- Existing CLI flags, configuration JSON, and audio/video format names remain compatible.
