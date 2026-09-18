/// Text Post-Processing Pipeline inspired by Umi-OCR.
/// Handles OCR-induced formatting artifacts such as hyphenation across line breaks,
/// fragmented paragraph lines, and mixed CJK/Latin spacing.

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' | // CJK Unified Ideographs Extension A
        '\u{F900}'..='\u{FAFF}' | // CJK Compatibility Ideographs
        '\u{3040}'..='\u{309F}' | // Hiragana
        '\u{30A0}'..='\u{30FF}' | // Katakana
        '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
    )
}

fn is_sentence_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | ';' | ':' | '。' | '！' | '？' | '；' | '：')
}

/// Strip trailing hyphenation from words broken across lines (e.g. "inter-\nnational" -> "international").
pub fn clean_hyphenation(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if (chars[i] == '-' || chars[i] == '\u{00AD}') && i > 0 && chars[i - 1].is_alphabetic() {
            // Look ahead past whitespace and at least one newline
            let mut j = i + 1;
            while j < len && (chars[j] == ' ' || chars[j] == '\t' || chars[j] == '\r') {
                j += 1;
            }
            if j < len && chars[j] == '\n' {
                j += 1;
                while j < len && (chars[j] == ' ' || chars[j] == '\t' || chars[j] == '\r') {
                    j += 1;
                }
                if j < len && chars[j].is_alphabetic() {
                    // Hyphen broken across line detected: omit hyphen and line break
                    i = j;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Merge lines that were artificially broken by OCR layout without sentence terminators.
pub fn merge_paragraphs(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(text.len());

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            result.push('\n');
            result.push('\n');
            continue;
        }

        result.push_str(trimmed);

        if idx + 1 < lines.len() {
            let next_line = lines[idx + 1].trim();
            if next_line.is_empty() {
                continue;
            }

            let last_char = trimmed.chars().last().unwrap_or(' ');
            let next_first_char = next_line.chars().next().unwrap_or(' ');

            if is_sentence_terminator(last_char) {
                // Natural sentence boundary: preserve line break
                result.push('\n');
            } else if is_cjk(last_char) && is_cjk(next_first_char) {
                // Continuous CJK without punctuation: merge directly without space
            } else {
                // Latin/number continuation: join with a single space
                result.push(' ');
            }
        }
    }

    result
}

/// Standardize spacing between CJK and ASCII alphanumeric characters (e.g. "PDF工具2026版" -> "PDF 工具 2026 版").
pub fn normalize_cjk_spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(text.len() + 32);

    for i in 0..chars.len() {
        result.push(chars[i]);
        if i + 1 < chars.len() {
            let curr = chars[i];
            let next = chars[i + 1];

            let cjk_then_alnum = is_cjk(curr) && (next.is_ascii_alphanumeric());
            let alnum_then_cjk = (curr.is_ascii_alphanumeric()) && is_cjk(next);

            if cjk_then_alnum || alnum_then_cjk {
                result.push(' ');
            }
        }
    }

    result
}

/// Check if a text segment contains LaTeX math formulas or special scientific operators.
pub fn is_math_expression(text: &str) -> bool {
    let t = text.trim();
    if (t.starts_with('$') && t.ends_with('$')) || (t.starts_with("\\(") && t.ends_with("\\)")) {
        return true;
    }
    const MATH_KEYWORDS: &[&str] = &[
        "\\frac", "\\sum", "\\int", "\\sqrt", "\\prod", "\\lim", "\\alpha", "\\beta",
        "\\gamma", "\\theta", "\\lambda", "\\sigma", "\\infty", "\\approx", "\\neq",
        "\\le", "\\ge", "\\pm", "\\times", "\\div", "\\partial", "\\nabla", "\\in",
        "\\subset", "\\cup", "\\cap",
    ];
    for kw in MATH_KEYWORDS {
        if text.contains(kw) {
            return true;
        }
    }
    // Greek and math operator symbols
    let has_math_char = text.chars().any(|c| matches!(c,
        '±' | '×' | '÷' | '≠' | '≤' | '≥' | '≈' | '∑' | '∏' | '∫' | '∂' | '∇' | '∞' |
        'α'..='ω' | 'Α'..='Ω'
    ));
    has_math_char && (text.contains('^') || text.contains('_') || text.contains('=') || text.contains('/'))
}

/// Comprehensive OCR text cleaning pipeline with math formula preservation.
pub fn clean_ocr_text(raw_text: &str) -> String {
    if is_math_expression(raw_text) {
        // Preserve math formulas intact
        return raw_text.trim().to_string();
    }
    let unhyphenated = clean_hyphenation(raw_text);
    let merged = merge_paragraphs(&unhyphenated);
    normalize_cjk_spacing(&merged)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    pub points: Vec<[f32; 2]>,
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusteredParagraph {
    pub points: Vec<[f32; 2]>,
    pub text: String,
    pub confidence: f32,
    pub line_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTable {
    pub rows: Vec<Vec<String>>,
    pub markdown: String,
    pub csv: String,
}

fn get_rect_bounds(points: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    if points.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = points[0][0];
    let mut max_x = points[0][0];
    let mut min_y = points[0][1];
    let mut max_y = points[0][1];
    for pt in points {
        if pt[0] < min_x { min_x = pt[0]; }
        if pt[0] > max_x { max_x = pt[0]; }
        if pt[1] < min_y { min_y = pt[1]; }
        if pt[1] > max_y { max_y = pt[1]; }
    }
    (min_x, min_y, max_x, max_y)
}

/// Cluster fragmented OCR bounding boxes into cohesive paragraph blocks.
/// Lines with close vertical proximity and aligned margins are clustered together.
pub fn cluster_ocr_boxes(boxes: &[BoundingBox]) -> Vec<ClusteredParagraph> {
    if boxes.is_empty() {
        return Vec::new();
    }

    // Sort boxes primarily by vertical Y position, secondarily by horizontal X
    let mut sorted: Vec<(usize, &BoundingBox, (f32, f32, f32, f32))> = boxes
        .iter()
        .enumerate()
        .map(|(idx, b)| (idx, b, get_rect_bounds(&b.points)))
        .collect();

    sorted.sort_by(|a, b| {
        let y_diff = a.2 .1.partial_cmp(&b.2 .1).unwrap_or(std::cmp::Ordering::Equal);
        if y_diff == std::cmp::Ordering::Equal {
            a.2 .0.partial_cmp(&b.2 .0).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            y_diff
        }
    });

    let mut clusters: Vec<Vec<&BoundingBox>> = Vec::new();

    for (_, bbox, bounds) in sorted {
        let (min_x, min_y, _max_x, _max_y) = bounds;
        let _height = (bounds.3 - bounds.1).max(8.0);

        let mut joined = false;
        // Check if this box can append to the last cluster
        if let Some(last_cluster) = clusters.last_mut() {
            if let Some(last_box) = last_cluster.last() {
                let last_bounds = get_rect_bounds(&last_box.points);
                let last_height = (last_bounds.3 - last_bounds.1).max(8.0);
                let vertical_gap = min_y - last_bounds.3;

                // Vertical gap within 1.6x line height and horizontal overlap or left margin alignment
                let x_margin_diff = (min_x - last_bounds.0).abs();
                let is_vertically_adjacent = vertical_gap >= -4.0 && vertical_gap <= last_height * 1.6;
                let is_horizontally_aligned = x_margin_diff <= last_height * 3.0 || min_x <= last_bounds.2;

                if is_vertically_adjacent && is_horizontally_aligned {
                    last_cluster.push(bbox);
                    joined = true;
                }
            }
        }

        if !joined {
            clusters.push(vec![bbox]);
        }
    }

    clusters
        .into_iter()
        .map(|cluster| {
            let mut all_points: Vec<[f32; 2]> = Vec::new();
            let mut combined_text = String::new();
            let mut total_conf = 0.0;

            for (i, b) in cluster.iter().enumerate() {
                all_points.extend(b.points.iter().copied());
                total_conf += b.confidence;
                if i > 0 {
                    combined_text.push('\n');
                }
                combined_text.push_str(&b.text);
            }

            let (min_x, min_y, max_x, max_y) = get_rect_bounds(&all_points);
            let cleaned = clean_ocr_text(&combined_text);
            let avg_conf = total_conf / (cluster.len() as f32).max(1.0);

            ClusteredParagraph {
                points: vec![
                    [min_x, min_y],
                    [max_x, min_y],
                    [max_x, max_y],
                    [min_x, max_y],
                ],
                text: cleaned,
                confidence: avg_conf,
                line_count: cluster.len(),
            }
        })
        .collect()
}

/// Detect tabular structure from OCR boxes and produce Markdown / CSV representations.
pub fn detect_tables(boxes: &[BoundingBox]) -> Vec<DetectedTable> {
    if boxes.is_empty() {
        return Vec::new();
    }

    // Extract item with bounds
    let mut items: Vec<(&BoundingBox, (f32, f32, f32, f32), f32)> = boxes
        .iter()
        .map(|b| {
            let bounds = get_rect_bounds(&b.points);
            let mid_y = (bounds.1 + bounds.3) / 2.0;
            (b, bounds, mid_y)
        })
        .collect();

    // Sort by mid_y
    items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Group items into rows by vertical proximity
    let mut rows: Vec<Vec<(&BoundingBox, (f32, f32, f32, f32))>> = Vec::new();
    for (bbox, bounds, mid_y) in items {
        let h = (bounds.3 - bounds.1).max(10.0);
        let mut added = false;
        for row in &mut rows {
            let row_mid_y = (row[0].1 .1 + row[0].1 .3) / 2.0;
            if (mid_y - row_mid_y).abs() <= h * 0.5 {
                row.push((bbox, bounds));
                added = true;
                break;
            }
        }
        if !added {
            rows.push(vec![(bbox, bounds)]);
        }
    }

    // Sort each row left-to-right
    for row in &mut rows {
        row.sort_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Find consecutive rows that have multiple columns (table candidate)
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let multi_col_rows: Vec<&Vec<(&BoundingBox, (f32, f32, f32, f32))>> = rows
        .iter()
        .filter(|r| r.len() >= 2)
        .collect();

    if multi_col_rows.len() < 2 {
        return Vec::new();
    }

    // Determine max columns
    let max_cols = multi_col_rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols < 2 {
        return Vec::new();
    }

    for row in &multi_col_rows {
        let mut row_strings = Vec::with_capacity(max_cols);
        for item in *row {
            row_strings.push(item.0.text.trim().to_string());
        }
        while row_strings.len() < max_cols {
            row_strings.push(String::new());
        }
        table_rows.push(row_strings);
    }

    // Generate Markdown table
    let mut md = String::new();
    if let Some(header) = table_rows.first() {
        md.push_str("| ");
        md.push_str(&header.join(" | "));
        md.push_str(" |\n");

        md.push_str("| ");
        let sep: Vec<&str> = vec!["---"; max_cols];
        md.push_str(&sep.join(" | "));
        md.push_str(" |\n");

        for r in table_rows.iter().skip(1) {
            md.push_str("| ");
            md.push_str(&r.join(" | "));
            md.push_str(" |\n");
        }
    }

    // Generate CSV table
    let mut csv = String::new();
    for r in &table_rows {
        let escaped: Vec<String> = r
            .iter()
            .map(|cell| {
                if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                    format!("\"{}\"", cell.replace('"', "\"\""))
                } else {
                    cell.clone()
                }
            })
            .collect();
        csv.push_str(&escaped.join(","));
        csv.push('\n');
    }

    vec![DetectedTable {
        rows: table_rows,
        markdown: md,
        csv,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_hyphenation() {
        let input = "The inter-\n   national organization was founded.";
        assert_eq!(clean_hyphenation(input), "The international organization was founded.");

        let compound = "state-of-the-art technology";
        assert_eq!(clean_hyphenation(compound), "state-of-the-art technology");
    }

    #[test]
    fn test_merge_paragraphs_latin() {
        let input = "This is a single sentence that was split\nacross two lines by the scanner.\nAnd this is a new sentence.";
        let res = merge_paragraphs(input);
        assert!(res.contains("This is a single sentence that was split across two lines by the scanner."));
        assert!(res.contains("And this is a new sentence."));
    }

    #[test]
    fn test_merge_paragraphs_cjk() {
        let input = "这是第一行中文内容需要继续\n拼接在一起而不要产生多余空格。这是下一句。";
        let res = merge_paragraphs(input);
        assert!(res.contains("这是第一行中文内容需要继续拼接在一起而不要产生多余空格。"));
    }

    #[test]
    fn test_normalize_cjk_spacing() {
        let input = "PDF工具箱2026版本在macOS运行良好";
        let res = normalize_cjk_spacing(input);
        assert_eq!(res, "PDF 工具箱 2026 版本在 macOS 运行良好");
    }

    #[test]
    fn test_math_formula_preservation() {
        let math_expr = r"E = mc^2 \quad \int_{0}^{\infty} e^{-x^2} dx = \frac{\sqrt{\pi}}{2}";
        assert!(is_math_expression(math_expr));
        assert_eq!(clean_ocr_text(math_expr), math_expr);
    }

    #[test]
    fn test_cluster_ocr_boxes() {
        let b1 = BoundingBox {
            points: vec![[10.0, 10.0], [100.0, 10.0], [100.0, 25.0], [10.0, 25.0]],
            text: "This is the first line of a".to_string(),
            confidence: 0.95,
        };
        let b2 = BoundingBox {
            points: vec![[10.0, 30.0], [110.0, 30.0], [110.0, 45.0], [10.0, 45.0]],
            text: "paragraph that continues here.".to_string(),
            confidence: 0.96,
        };
        let clusters = cluster_ocr_boxes(&[b1, b2]);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].line_count, 2);
        assert!(clusters[0].text.contains("This is the first line of a paragraph that continues here."));
    }

    #[test]
    fn test_detect_tables() {
        let c11 = BoundingBox { points: vec![[10.0, 10.0], [50.0, 10.0], [50.0, 25.0], [10.0, 25.0]], text: "Name".to_string(), confidence: 0.99 };
        let c12 = BoundingBox { points: vec![[60.0, 10.0], [100.0, 10.0], [100.0, 25.0], [60.0, 25.0]], text: "Age".to_string(), confidence: 0.99 };
        let c21 = BoundingBox { points: vec![[10.0, 30.0], [50.0, 30.0], [50.0, 45.0], [10.0, 45.0]], text: "Alice".to_string(), confidence: 0.98 };
        let c22 = BoundingBox { points: vec![[60.0, 30.0], [100.0, 30.0], [100.0, 45.0], [60.0, 45.0]], text: "30".to_string(), confidence: 0.98 };

        let tables = detect_tables(&[c11, c12, c21, c22]);
        assert_eq!(tables.len(), 1);
        assert!(tables[0].markdown.contains("| Name | Age |"));
        assert!(tables[0].markdown.contains("| Alice | 30 |"));
        assert!(tables[0].csv.contains("Name,Age"));
        assert!(tables[0].csv.contains("Alice,30"));
    }
}

