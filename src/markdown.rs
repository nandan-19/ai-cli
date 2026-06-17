/// Process inline Markdown spans into ANSI-escaped text.
/// Handles: **bold**, *italic*, ***bold italic***, `code`, ~~strikethrough~~,
///           [link](url), ![img](url), <autolink>, \escape
pub fn render_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Backslash escape
        if chars[i] == '\\' && i + 1 < len {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }

        // Autolink  <url> or <email>
        if chars[i] == '<' {
            if let Some(end) = chars[i..].iter().position(|&c| c == '>') {
                let url: String = chars[i + 1..i + end].iter().collect();
                if url.contains("://") || url.contains('@') {
                    out.push_str(&format!("\x1b[4;34m{}\x1b[0m", url));
                    i += end + 1;
                    continue;
                }
            }
        }

        // Image  ![alt](url)
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            if let Some(bracket_end) = chars[i + 2..].iter().position(|&c| c == ']') {
                let alt_end = i + 2 + bracket_end;
                if alt_end + 1 < len && chars[alt_end + 1] == '(' {
                    if let Some(paren_end) = chars[alt_end + 2..].iter().position(|&c| c == ')') {
                        let url: String =
                            chars[alt_end + 2..alt_end + 2 + paren_end].iter().collect();
                        let alt: String = chars[i + 2..alt_end].iter().collect();
                        out.push_str(&format!("\x1b[35m🖼 {}\x1b[0m \x1b[2m({})\x1b[0m", alt, url));
                        i = alt_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // Link  [text](url)
        if chars[i] == '[' {
            if let Some(bracket_end) = chars[i + 1..].iter().position(|&c| c == ']') {
                let text_end = i + 1 + bracket_end;
                if text_end + 1 < len && chars[text_end + 1] == '(' {
                    if let Some(paren_end) = chars[text_end + 2..].iter().position(|&c| c == ')') {
                        let url: String = chars[text_end + 2..text_end + 2 + paren_end]
                            .iter()
                            .collect();
                        let link_text: String = chars[i + 1..text_end].iter().collect();
                        let rendered_text = render_inline(&link_text);
                        out.push_str(&format!(
                            "\x1b[4;34m{}\x1b[0m \x1b[2m({})\x1b[0m",
                            rendered_text, url
                        ));
                        i = text_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // Code span  `code`  (may use multiple backticks)
        if chars[i] == '`' {
            let tick_start = i;
            while i < len && chars[i] == '`' {
                i += 1;
            }
            let tick_count = i - tick_start;
            let closing: String = std::iter::repeat('`').take(tick_count).collect();
            let rest: String = chars[i..].iter().collect();
            if let Some(end_pos) = rest.find(&closing) {
                let code = &rest[..end_pos];
                out.push_str(&format!("\x1b[96m{}\x1b[0m", code));
                i += end_pos + tick_count;
                continue;
            } else {
                // No closing — treat as literal
                for _ in 0..tick_count {
                    out.push('`');
                }
                continue;
            }
        }

        // ~~strikethrough~~
        if chars[i] == '~' && i + 1 < len && chars[i + 1] == '~' {
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == '~' && i + 1 < len && chars[i + 1] == '~' {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[9m{}\x1b[29m", word));
            continue;
        }

        // ==highlight==
        if chars[i] == '=' && i + 1 < len && chars[i + 1] == '=' {
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == '=' && i + 1 < len && chars[i + 1] == '=' {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[93;1m{}\x1b[0m", word));
            continue;
        }

        // ***bold italic***
        if chars[i] == '*' && i + 2 < len && chars[i + 1] == '*' && chars[i + 2] == '*' {
            i += 3;
            let mut word = String::new();
            while i < len {
                if chars[i] == '*' && i + 2 < len && chars[i + 1] == '*' && chars[i + 2] == '*' {
                    i += 3;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[1;3m{}\x1b[0m", word));
            continue;
        }

        // **bold** or __bold__
        if (chars[i] == '*' && i + 1 < len && chars[i + 1] == '*')
            || (chars[i] == '_' && i + 1 < len && chars[i + 1] == '_')
        {
            let delim = chars[i];
            i += 2;
            let mut word = String::new();
            while i < len {
                if chars[i] == delim && i + 1 < len && chars[i + 1] == delim {
                    i += 2;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[1m{}\x1b[22m", word));
            continue;
        }

        // *italic* or _italic_
        if chars[i] == '*' || chars[i] == '_' {
            let delim = chars[i];
            i += 1;
            let mut word = String::new();
            while i < len {
                if chars[i] == delim {
                    i += 1;
                    break;
                }
                word.push(chars[i]);
                i += 1;
            }
            out.push_str(&format!("\x1b[3m{}\x1b[23m", word));
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Render a full Markdown document to styled ANSI terminal output.
/// Covers: headings H1-H6, thematic breaks, fenced code blocks (``` and ~~~),
/// indented code blocks, blockquotes, ordered/unordered/task lists,
/// GFM tables, and all inline formatting via render_inline.
pub fn render_markdown(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let mut i = 0;

    // Terminal width for rulers (default 72 if unable to detect)
    let term_width: usize = 72;

    while i < total {
        let line = lines[i];
        let trimmed = line.trim();

        // ── Fenced code block ─────────────────────────────────────────────
        // Supports both ``` and ~~~
        let fence_char = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };

        if let Some(fc) = fence_char {
            let fence_prefix: String = std::iter::repeat(fc).take(3).collect();
            let lang = trimmed.trim_start_matches(fc).trim();
            if !lang.is_empty() {
                println!("\x1b[2m[{}]\x1b[0m", lang);
            }
            i += 1;
            while i < total {
                let code_line = lines[i];
                if code_line.trim().starts_with(&fence_prefix) {
                    i += 1;
                    break;
                }
                println!(
                    "\x1b[48;5;235m\x1b[93m {:<width$}\x1b[0m",
                    code_line,
                    width = term_width.saturating_sub(1)
                );
                i += 1;
            }
            println!();
            continue;
        }

        // ── Indented code block (4+ spaces or 1 tab) ────────────────────
        if line.starts_with("    ") || line.starts_with('\t') {
            let code = if line.starts_with('\t') {
                &line[1..]
            } else {
                &line[4..]
            };
            println!(
                "\x1b[48;5;235m\x1b[93m {:<width$}\x1b[0m",
                code,
                width = term_width.saturating_sub(1)
            );
            i += 1;
            continue;
        }

        // ── Thematic break  --- / *** / ___ ─────────────────────────────
        let compact = trimmed.replace(' ', "");
        if compact == "---"
            || compact == "***"
            || compact == "___"
            || compact == "----"
            || compact == "====="
        {
            println!("\x1b[2m{}\x1b[0m", "─".repeat(term_width));
            i += 1;
            continue;
        }

        // ── ATX Headings  # through ###### ──────────────────────────────
        let heading_level = trimmed.chars().take_while(|&c| c == '#').count();
        if heading_level > 0
            && heading_level <= 6
            && trimmed.len() > heading_level
            && trimmed.as_bytes().get(heading_level) == Some(&b' ')
        {
            let content = &trimmed[heading_level + 1..];
            let rendered = render_inline(content);
            match heading_level {
                1 => {
                    let bar = "═".repeat(term_width);
                    println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                    println!("\x1b[1;38;5;220m  {}\x1b[0m", rendered);
                    println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                }
                2 => {
                    println!("\x1b[1;38;5;214m▌ {}\x1b[0m", rendered);
                    println!("\x1b[38;5;214m{}\x1b[0m", "─".repeat(term_width));
                }
                3 => println!("\x1b[1;38;5;208m◆ {}\x1b[0m", rendered),
                4 => println!("\x1b[1;38;5;203m● {}\x1b[0m", rendered),
                5 => println!("\x1b[1;38;5;198m○ {}\x1b[0m", rendered),
                6 => println!("\x1b[1;38;5;176m· {}\x1b[0m", rendered),
                _ => println!("{}", rendered),
            }
            i += 1;
            continue;
        }

        // ── Setext heading  (underlined with === or ---) ─────────────────
        if i + 1 < total {
            let next = lines[i + 1].trim();
            if !trimmed.is_empty() && (next.chars().all(|c| c == '=') && !next.is_empty()) {
                let rendered = render_inline(trimmed);
                let bar = "═".repeat(term_width);
                println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                println!("\x1b[1;38;5;220m  {}\x1b[0m", rendered);
                println!("\x1b[1;38;5;220m{}\x1b[0m", bar);
                i += 2;
                continue;
            }
            if !trimmed.is_empty() && (next.chars().all(|c| c == '-') && next.len() > 1) {
                let rendered = render_inline(trimmed);
                println!("\x1b[1;38;5;214m▌ {}\x1b[0m", rendered);
                println!("\x1b[38;5;214m{}\x1b[0m", "─".repeat(term_width));
                i += 2;
                continue;
            }
        }

        // ── Blockquote  > ... ────────────────────────────────────────────
        if trimmed.starts_with("> ") || trimmed == ">" {
            let content = if trimmed == ">" { "" } else { &trimmed[2..] };
            let rendered = render_inline(content);
            println!("\x1b[38;5;244m│\x1b[0m \x1b[3;38;5;252m{}\x1b[0m", rendered);
            i += 1;
            continue;
        }

        // ── GFM Table ────────────────────────────────────────────────────
        // Detect: line has | chars and next line is a separator row
        if trimmed.starts_with('|') || trimmed.contains(" | ") {
            // Peek ahead to see if next line is a separator (---|--- pattern)
            let is_table_header = i + 1 < total && {
                let sep = lines[i + 1].trim();
                sep.starts_with('|')
                    && sep.contains('-')
                    && sep.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
            };

            if is_table_header {
                // Collect all table rows
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut sep_idx = 0_usize;
                let mut j = i;
                while j < total {
                    let row = lines[j].trim();
                    if row.is_empty() {
                        break;
                    }
                    if row.starts_with('|') || row.contains('|') {
                        let is_sep = row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
                        if is_sep {
                            sep_idx = rows.len();
                            rows.push(vec![]); // placeholder
                        } else {
                            let cells: Vec<String> = row
                                .split('|')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            rows.push(cells);
                        }
                    } else {
                        break;
                    }
                    j += 1;
                }

                // Compute column widths
                let col_count = rows
                    .iter()
                    .filter(|r| !r.is_empty())
                    .map(|r| r.len())
                    .max()
                    .unwrap_or(0);
                let mut col_widths = vec![3usize; col_count];
                for row in &rows {
                    for (ci, cell) in row.iter().enumerate() {
                        if ci < col_count {
                            col_widths[ci] = col_widths[ci].max(cell.len());
                        }
                    }
                }

                // Draw top border
                let top: String = col_widths
                    .iter()
                    .map(|&w| "─".repeat(w + 2))
                    .collect::<Vec<_>>()
                    .join("┬");
                println!("\x1b[38;5;244m┌{}┐\x1b[0m", top);

                let mut is_first = true;
                for (ri, row) in rows.iter().enumerate() {
                    if ri == sep_idx {
                        // separator row → draw mid-border
                        let mid: String = col_widths
                            .iter()
                            .map(|&w| "─".repeat(w + 2))
                            .collect::<Vec<_>>()
                            .join("┼");
                        println!("\x1b[38;5;244m├{}┤\x1b[0m", mid);
                        continue;
                    }
                    if row.is_empty() {
                        continue;
                    }

                    let row_str: String = row
                        .iter()
                        .enumerate()
                        .map(|(ci, cell)| {
                            let w = *col_widths.get(ci).unwrap_or(&3);
                            let rendered = render_inline(cell);
                            if is_first {
                                format!(" \x1b[1;36m{:<width$}\x1b[0m ", rendered, width = w)
                            } else {
                                format!(" {:<width$} ", rendered, width = w)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\x1b[38;5;244m│\x1b[0m");
                    println!("\x1b[38;5;244m│\x1b[0m{}\x1b[38;5;244m│\x1b[0m", row_str);
                    is_first = false;
                }

                // Draw bottom border
                let bot: String = col_widths
                    .iter()
                    .map(|&w| "─".repeat(w + 2))
                    .collect::<Vec<_>>()
                    .join("┴");
                println!("\x1b[38;5;244m└{}┘\x1b[0m", bot);
                println!();
                i = j;
                continue;
            }
        }

        // ── Task list  - [ ] / - [x] ─────────────────────────────────────
        if trimmed.starts_with("- [ ] ") || trimmed.starts_with("* [ ] ") {
            let content = render_inline(&trimmed[6..]);
            println!("  \x1b[38;5;244m☐\x1b[0m {}", content);
            i += 1;
            continue;
        }
        if trimmed.starts_with("- [x] ")
            || trimmed.starts_with("* [x] ")
            || trimmed.starts_with("- [X] ")
            || trimmed.starts_with("* [X] ")
        {
            let content = render_inline(&trimmed[6..]);
            println!("  \x1b[32m☑\x1b[0m \x1b[2m{}\x1b[0m", content);
            i += 1;
            continue;
        }

        // ── Unordered list  - / * / + ────────────────────────────────────
        let indent_spaces = line.len() - line.trim_start().len();
        let is_unordered =
            trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ");
        if is_unordered {
            let content = render_inline(&trimmed[2..]);
            let indent = "  ".repeat(indent_spaces / 2);
            let bullet = match indent_spaces / 2 {
                0 => "\x1b[36m•\x1b[0m",
                1 => "\x1b[35m◦\x1b[0m",
                _ => "\x1b[34m▸\x1b[0m",
            };
            println!("{}  {} {}", indent, bullet, content);
            i += 1;
            continue;
        }

        // ── Ordered list  1. 2. etc ─────────────────────────────────────
        let is_ordered = {
            let mut dot_pos = None;
            for (ci, ch) in trimmed.char_indices() {
                if ch == '.' || ch == ')' {
                    dot_pos = Some(ci);
                    break;
                }
                if !ch.is_ascii_digit() {
                    break;
                }
            }
            dot_pos
                .map(|dp| trimmed[..dp].parse::<u32>().is_ok() && trimmed.len() > dp + 1)
                .unwrap_or(false)
        };
        if is_ordered {
            let dot_pos = trimmed.find('.').or_else(|| trimmed.find(')')).unwrap_or(0);
            let num = &trimmed[..dot_pos];
            let content = render_inline(trimmed[dot_pos + 1..].trim_start());
            let indent = "  ".repeat(indent_spaces / 2);
            println!("{}  \x1b[1;36m{}.\x1b[0m {}", indent, num, content);
            i += 1;
            continue;
        }

        // ── Blank line ───────────────────────────────────────────────────
        if trimmed.is_empty() {
            println!();
            i += 1;
            continue;
        }

        // ── Plain paragraph ──────────────────────────────────────────────
        println!("{}", render_inline(trimmed));
        i += 1;
    }
    println!();
}
