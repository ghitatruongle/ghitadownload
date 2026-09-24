# Changelog

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
