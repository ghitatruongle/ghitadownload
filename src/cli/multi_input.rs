use anyhow::Result;
use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType},
};
use std::io::{stdout, Write};
use std::path::Path;
use std::time::Duration;

pub struct MultiLinkInput;

impl MultiLinkInput {
    pub fn prompt() -> Result<Option<Vec<String>>> {
        terminal::enable_raw_mode()?;
        let mut stdout = stdout();

        let mut links: Vec<String> = vec![String::new()];
        let mut current_idx: usize = 0;
        let mut cursor_char_pos: usize = 0;
        let mut status_message: Option<String> = None;

        let res = Self::event_loop(
            &mut stdout,
            &mut links,
            &mut current_idx,
            &mut cursor_char_pos,
            &mut status_message,
        );

        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout, cursor::Show);
        println!();

        match res {
            Ok(Some(raw_list)) => {
                let expanded = Self::process_links(raw_list);
                Ok(Some(expanded))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn event_loop(
        stdout: &mut std::io::Stdout,
        links: &mut Vec<String>,
        current_idx: &mut usize,
        cursor_char_pos: &mut usize,
        status_message: &mut Option<String>,
    ) -> Result<Option<Vec<String>>> {
        let mut last_rendered_lines: u16 = 0;

        loop {
            last_rendered_lines = Self::render(
                stdout,
                links,
                *current_idx,
                *cursor_char_pos,
                status_message.as_deref(),
                last_rendered_lines,
            )?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key_event) = event::read()? {
                    *status_message = None;

                    match key_event {
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            return Ok(None);
                        }

                        KeyEvent {
                            code: KeyCode::Down,
                            ..
                        } => {
                            if *current_idx + 1 < links.len() {
                                *current_idx += 1;
                                *cursor_char_pos = links[*current_idx].chars().count();
                            } else if !links[*current_idx].trim().is_empty() {
                                links.push(String::new());
                                *current_idx += 1;
                                *cursor_char_pos = 0;
                            } else {
                                *status_message = Some(
                                    "Dòng hiện tại đang trống. Nhập link rồi nhấn [↓] tiếp."
                                        .to_string(),
                                );
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Up, ..
                        } => {
                            if *current_idx > 0 {
                                *current_idx -= 1;
                                *cursor_char_pos = links[*current_idx].chars().count();
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Left,
                            ..
                        } => {
                            if *cursor_char_pos > 0 {
                                *cursor_char_pos -= 1;
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Right,
                            ..
                        } => {
                            let char_count = links[*current_idx].chars().count();
                            if *cursor_char_pos < char_count {
                                *cursor_char_pos += 1;
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Home,
                            ..
                        } => {
                            *cursor_char_pos = 0;
                        }

                        KeyEvent {
                            code: KeyCode::End, ..
                        } => {
                            *cursor_char_pos = links[*current_idx].chars().count();
                        }

                        KeyEvent {
                            code: KeyCode::Backspace,
                            ..
                        } => {
                            if *cursor_char_pos > 0 {
                                let mut chars: Vec<char> = links[*current_idx].chars().collect();
                                chars.remove(*cursor_char_pos - 1);
                                links[*current_idx] = chars.into_iter().collect();
                                *cursor_char_pos -= 1;
                            } else if links.len() > 1 && links[*current_idx].is_empty() {
                                links.remove(*current_idx);
                                if *current_idx >= links.len() {
                                    *current_idx = links.len() - 1;
                                }
                                *cursor_char_pos = links[*current_idx].chars().count();
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Delete,
                            ..
                        } => {
                            let mut chars: Vec<char> = links[*current_idx].chars().collect();
                            if *cursor_char_pos < chars.len() {
                                chars.remove(*cursor_char_pos);
                                links[*current_idx] = chars.into_iter().collect();
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            let has_more_buffered = event::poll(Duration::from_millis(5))?;
                            if has_more_buffered {
                                if *current_idx + 1 < links.len() {
                                    *current_idx += 1;
                                    *cursor_char_pos = links[*current_idx].chars().count();
                                } else {
                                    links.push(String::new());
                                    *current_idx += 1;
                                    *cursor_char_pos = 0;
                                }
                            } else {
                                let valid_links: Vec<String> = links
                                    .iter()
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                if valid_links.is_empty() {
                                    *status_message = Some(
                                        "⚠ Vui lòng nhập ít nhất 1 liên kết hợp lệ!".to_string(),
                                    );
                                } else {
                                    return Ok(Some(valid_links));
                                }
                            }
                        }

                        KeyEvent {
                            code: KeyCode::Char(c),
                            modifiers,
                            ..
                        } if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
                            if c == '\n' || c == '\r' {
                                links.push(String::new());
                                *current_idx += 1;
                                *cursor_char_pos = 0;
                            } else {
                                let mut chars: Vec<char> = links[*current_idx].chars().collect();
                                if *cursor_char_pos <= chars.len() {
                                    chars.insert(*cursor_char_pos, c);
                                    links[*current_idx] = chars.into_iter().collect();
                                    *cursor_char_pos += 1;
                                }
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
    }

    fn render(
        stdout: &mut std::io::Stdout,
        links: &[String],
        current_idx: usize,
        cursor_char_pos: usize,
        status_message: Option<&str>,
        last_rendered_lines: u16,
    ) -> Result<u16> {
        let (term_cols, _) = terminal::size().unwrap_or((80, 24));
        let max_content_width = (term_cols as usize).saturating_sub(22).max(20);

        if last_rendered_lines > 0 {
            execute!(
                stdout,
                cursor::MoveUp(last_rendered_lines),
                cursor::MoveToColumn(0)
            )?;
        }
        execute!(stdout, Clear(ClearType::FromCursorDown))?;

        let mut lines_drawn: u16 = 0;

        let header_border = "═".repeat(term_cols.saturating_sub(2) as usize);
        println!("╔{}╗", header_border);
        println!(
            "║ {:<width$} ║",
            "NHẬP DANH SÁCH LIÊN KẾT (HỖ TRỢ NHIỀU LIÊN KẾT LIÊN TIẾP)"
                .cyan()
                .bold(),
            width = term_cols.saturating_sub(4) as usize
        );
        println!("╚{}╝", header_border);
        println!(
            " {}",
            "• Nhập link rồi ấn [Mũi tên XUỐNG ↓] để nhập liên kết thứ 2, thứ 3...".yellow()
        );
        println!(
            " {}",
            "• Dùng [↑] và [↓] để di chuyển giữa các dòng | [Backspace] để xóa dòng trống"
                .bright_black()
        );
        println!(
            " {}",
            "• Hỗ trợ dán (Ctrl+V) hàng loạt hoặc nhập file .txt (ví dụ: links.txt)".bright_black()
        );
        println!(
            " {}",
            "• Nhấn [ENTER] khi hoàn tất để bắt đầu tải | [ESC] để quay lại"
                .green()
                .bold()
        );
        println!("{}", "-".repeat(term_cols as usize));
        lines_drawn += 8;

        let mut cursor_row_offset = 0;
        let mut cursor_col = 0;

        for (i, text) in links.iter().enumerate() {
            let prefix_num = format!("[#{}]", i + 1);
            let is_active = i == current_idx;

            let chars: Vec<char> = text.chars().collect();
            let char_len = chars.len();

            let (display_text, display_cursor_pos) = if is_active {
                if char_len <= max_content_width {
                    (text.clone(), cursor_char_pos)
                } else {
                    let start = if cursor_char_pos >= max_content_width {
                        cursor_char_pos + 1 - max_content_width
                    } else {
                        0
                    };
                    let end = (start + max_content_width).min(char_len);
                    let slice: String = chars[start..end].iter().collect();
                    (slice, cursor_char_pos.saturating_sub(start))
                }
            } else if char_len <= max_content_width {
                (text.clone(), 0)
            } else {
                let slice: String = chars[..max_content_width].iter().collect();
                (format!("{}...", slice), 0)
            };

            if is_active {
                cursor_row_offset = lines_drawn;
                let prompt_prefix = format!(
                    "  {} {:<5} > ",
                    "►".green().bold(),
                    prefix_num.cyan().bold()
                );
                print!("{}{}", prompt_prefix, display_text);
                println!();
                cursor_col = (11 + display_cursor_pos) as u16;
            } else {
                let prompt_prefix = format!("     {:<5} > ", prefix_num.bright_black());
                println!("{}{}", prompt_prefix, display_text.bright_white());
            }
            lines_drawn += 1;
        }

        if let Some(msg) = status_message {
            println!("  {}", msg.yellow().bold());
            lines_drawn += 1;
        } else {
            let total_valid = links.iter().filter(|s| !s.trim().is_empty()).count();
            println!(
                "  {}",
                format!(
                    "(Đã nhập: {} liên kết | Nhấn [ENTER] để tải ngay)",
                    total_valid
                )
                .bright_black()
            );
            lines_drawn += 1;
        }

        let lines_from_bottom = lines_drawn.saturating_sub(cursor_row_offset + 1);
        if lines_from_bottom > 0 {
            execute!(stdout, cursor::MoveUp(lines_from_bottom))?;
        }
        execute!(stdout, cursor::MoveToColumn(cursor_col))?;
        stdout.flush()?;

        Ok(lines_drawn)
    }

    pub fn process_links(inputs: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();

        for input in inputs {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            let path = Path::new(trimmed);
            if path.exists() && path.is_file() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for line in content.lines() {
                        let l = line.trim();
                        if !l.is_empty() && !l.starts_with('#') {
                            result.push(l.to_string());
                        }
                    }
                    continue;
                }
            }

            for item in trimmed.split([',', ';', '\n', '\r']) {
                let l = item.trim();
                if !l.is_empty() {
                    result.push(l.to_string());
                }
            }
        }

        result
    }
}
