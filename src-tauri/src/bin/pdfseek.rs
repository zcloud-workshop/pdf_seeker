//! pdfseek — pdf_seeker 的命令行核心动线（wedge MVP，2026-10-11）
//!
//! 用法：
//!   pdfseek <输入.pdf|png|jpg> [-o 输出.md] [--dpi 200] [--redact] [--models 目录] [--no-gpu]
//!
//! 动线：PDF(pdftoppm 光栅化)/图片 → PP-OCRv5 OCR → 行分组与段落重排 → Markdown
//! 可选 --redact：另存一份去除元数据（Info/XMP）的 PDF（<原名>.redacted.pdf）
//!
//! 设计约束（见档案 03-skills / 99-raw 二审定案 v2）：
//!   - 只做"文本提取+脱敏"，不做双层可搜索 PDF
//!   - 零服务器、零 API、纯本地

use anyhow::{bail, Context, Result};
use lopdf::Object;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

// ─── 参数 ─────────────────────────────────────────────────────────────

struct Args {
    input: PathBuf,
    out: Option<PathBuf>,
    dpi: u32,
    redact: bool,
    models: Option<PathBuf>,
    gpu: bool,
}

fn parse_args() -> Result<Args> {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("用法: pdfseek <输入.pdf|png|jpg> [-o 输出.md] [--dpi N] [--redact] [--models 目录] [--no-gpu]");
        std::process::exit(if argv.is_empty() { 2 } else { 0 });
    }
    let input = PathBuf::from(argv.remove(0));
    let mut a = Args { input, out: None, dpi: 200, redact: false, models: None, gpu: true };
    let mut it = argv.into_iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "-o" | "--out" => a.out = it.next().map(PathBuf::from),
            "--dpi" => a.dpi = it.next().context("--dpi 需要数值")?.parse().context("--dpi 非法")?,
            "--redact" => a.redact = true,
            "--models" => a.models = Some(PathBuf::from(it.next().context("--models 需要路径")?)),
            "--no-gpu" => a.gpu = false,
            other => bail!("未知参数: {}", other),
        }
    }
    Ok(a)
}

// ─── 模型定位 ─────────────────────────────────────────────────────────

fn find_models(explicit: &Option<PathBuf>) -> Result<PathBuf> {
    let candidates: Vec<PathBuf> = match explicit {
        Some(p) => vec![p.clone()],
        None => {
            let mut v = Vec::new();
            if let Ok(dir) = std::env::var("PDFSEEK_MODELS") {
                v.push(PathBuf::from(dir));
            }
            v.push(PathBuf::from("models/ocr")); // 运行于 src-tauri
            v.push(PathBuf::from("models")); // 发布包内平铺布局（binary 与 models/ 同级）
            v.push(PathBuf::from("src-tauri/models/ocr")); // 运行于项目根
            if let Some(exe) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
                v.push(exe.join("../models/ocr"));
            }
            v.push(PathBuf::from("../models/ocr"));
            v
        }
    };
    for dir in &candidates {
        if dir.join("PP-OCRv5_mobile_det.mnn").exists() {
            return Ok(dir.clone());
        }
    }
    bail!(
        "未找到 OCR 模型。用 --models 指定，或设置 PDFSEEK_MODELS。寻找过的位置：{:?}",
        candidates
    );
}

// ─── 输入 → 页图片列表 ────────────────────────────────────────────────

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg")
    )
}

fn which_pdftoppm() -> Option<()> {
    Command::new("pdftoppm")
        .arg("-v")
        .output()
        .map(|o| o.status.success() || !o.stderr.is_empty())
        .ok()
        .map(|_| ())
}

fn tempdir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pdfseek-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ─── OCR → 行/段落重组 ───────────────────────────────────────────────

struct Box_ {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    text: String,
}

/// 把 OCR 文本框重排为阅读顺序的行与段落：
/// 按 y 聚类成行（容差=中位行高的 60%），行内按 x 排序拼接；
/// 行距 > 1.8×行高 → 段落分隔。
fn boxes_to_markdown(boxes: &[Box_]) -> String {
    if boxes.is_empty() {
        return String::new();
    }
    let heights: Vec<f32> = boxes.iter().map(|b| b.y1 - b.y0).filter(|h| *h > 0.).collect();
    let median_h = {
        let mut h = heights;
        h.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        h.get(h.len() / 2).copied().unwrap_or(12.)
    };

    let mut sorted: Vec<&Box_> = boxes.iter().collect();
    sorted.sort_by(|a, b| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal));

    let mut lines: Vec<(f32, f32, Vec<&Box_>)> = Vec::new(); // (y_min, y_max, members)
    for b in sorted {
        let cy = (b.y0 + b.y1) / 2.;
        if let Some(last) = lines.last_mut() {
            let last_cy = (last.0 + last.1) / 2.;
            if (cy - last_cy).abs() <= median_h * 0.6 {
                last.0 = last.0.min(b.y0);
                last.1 = last.1.max(b.y1);
                last.2.push(b);
                continue;
            }
        }
        lines.push((b.y0, b.y1, vec![b]));
    }

    let mut out = String::new();
    let mut prev_y1: Option<f32> = None;
    for (i, (y0, y1, mut members)) in lines.into_iter().enumerate() {
        members.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        let mut line = String::new();
        for (j, m) in members.iter().enumerate() {
            if j == 0 {
                line.push_str(&m.text);
            } else {
                let prev = &members[j - 1].text;
                let prev_cjk = prev.chars().last().map(is_cjk).unwrap_or(false);
                let cur_cjk = m.text.chars().next().map(is_cjk).unwrap_or(false);
                // 西文词间补空格；CJK 直接拼接
                if !(prev_cjk || cur_cjk) {
                    line.push(' ');
                }
                // 词间距明显大于行高一半时，视为列间隔断
                if m.x0 - members[j - 1].x1 > median_h {
                    line.push_str("  ");
                }
                line.push_str(&m.text);
            }
        }
        let gap = prev_y1.map(|p| y0 - p).unwrap_or(0.);
        if i > 0 && gap > median_h * 1.8 {
            out.push_str("\n\n"); // 段落
        } else if i > 0 {
            out.push('\n');
        }
        out.push_str(&line);
        prev_y1 = Some(y1);
    }
    out
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3000..=0x303F | 0xFF00..=0xFFEF)
}

// ─── 元数据脱敏（lopdf） ─────────────────────────────────────────────

fn redact_pdf(src: &Path) -> Result<PathBuf> {
    let mut doc = lopdf::Document::load(src).context("打开 PDF 失败")?;
    // 1. 删除 Info 字典（作者/生成器/标题等）
    doc.trailer.remove(b"Info");
    // 2. 删除 catalog 上的 XMP Metadata 流引用
    if let Ok(root_obj) = doc.trailer.get(b"Root").cloned() {
        if let Object::Reference(root_id) = root_obj {
            if let Ok(Object::Dictionary(root)) = doc.get_object_mut(root_id) {
                root.remove(b"Metadata");
            }
        }
    }
    let out = src.with_file_name(format!(
        "{}.redacted.pdf",
        src.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
    ));
    doc.save(&out).context("保存脱敏 PDF 失败")?;
    Ok(out)
}

// ─── main ─────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("错误: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = parse_args()?;
    if !args.input.exists() {
        bail!("输入不存在: {}", args.input.display());
    }

    // 1. 收集页图片
    let cleanup_dir: Option<PathBuf>;
    let pages: Vec<PathBuf> = if is_image(&args.input) {
        cleanup_dir = None;
        vec![args.input.clone()]
    } else {
        let dir = pdf_to_pngs(&args.input, args.dpi)?;
        let mut pngs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .context("读取临时目录失败")?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
            .collect();
        pngs.sort(); // pdftoppm 输出 page-1.png / page-10.png：零填充保证字典序=页序
        if pngs.is_empty() {
            bail!("PDF 光栅化未产生任何页面");
        }
        cleanup_dir = Some(dir);
        pngs
    };

    // 2. 初始化 OCR 引擎
    let models = find_models(&args.models)?;
    let det = models.join("PP-OCRv5_mobile_det.mnn");
    let rec = models.join("PP-OCRv5_mobile_rec.mnn");
    let keys = models.join("ppocr_keys_v5.txt");
    let cfg = if args.gpu {
        Some(ocr_rs::OcrEngineConfig::new().with_backend(ocr_rs::Backend::Metal))
    } else {
        None
    };
    let engine = ocr_rs::OcrEngine::new(
        det.to_str().unwrap(),
        rec.to_str().unwrap(),
        keys.to_str().unwrap(),
        cfg,
    )
    .map_err(|e| anyhow::anyhow!("引擎初始化失败: {}", e))?;

    // 3. 逐页 OCR + 重排
    let t0 = Instant::now();
    let mut md = String::new();
    let total = pages.len();
    for (i, page) in pages.iter().enumerate() {
        let tp = Instant::now();
        let img = image::open(page).with_context(|| format!("加载页面图片 {}", page.display()))?;
        let results = engine
            .recognize(&img)
            .map_err(|e| anyhow::anyhow!("第 {} 页识别失败: {}", i + 1, e))?;
        let boxes: Vec<Box_> = results
            .iter()
            .map(|r| {
                let (x0, y0, x1, y1) = r
                    .bbox
                    .points
                    .as_ref()
                    .map(|pts| {
                        let xs = [pts[0].x, pts[1].x, pts[2].x, pts[3].x];
                        let ys = [pts[0].y, pts[1].y, pts[2].y, pts[3].y];
                        (
                            xs.iter().copied().fold(f32::INFINITY, |a, b| a.min(b)),
                            ys.iter().copied().fold(f32::INFINITY, |a, b| a.min(b)),
                            xs.iter().copied().fold(f32::NEG_INFINITY, |a, b| a.max(b)),
                            ys.iter().copied().fold(f32::NEG_INFINITY, |a, b| a.max(b)),
                        )
                    })
                    .unwrap_or_else(|| {
                        let rect = &r.bbox.rect;
                        (
                            rect.left() as f32,
                            rect.top() as f32,
                            rect.right() as f32,
                            rect.bottom() as f32,
                        )
                    });
                Box_ { x0, y0, x1, y1, text: r.text.clone() }
            })
            .collect();
        let n_chars: usize = boxes.iter().map(|b| b.text.chars().count()).sum();
        eprintln!(
            "  第 {}/{} 页：{} 个文本块 / {} 字 / {:.1}s",
            i + 1,
            total,
            boxes.len(),
            n_chars,
            tp.elapsed().as_secs_f32()
        );
        if total > 1 {
            md.push_str(&format!("\n\n<!-- 第 {} 页 -->\n\n", i + 1));
        }
        md.push_str(&boxes_to_markdown(&boxes));
    }
    md = md.trim().to_string();
    md.push('\n');

    // 4. 输出
    let out_path = args.out.clone().unwrap_or_else(|| {
        args.input.with_extension("md")
    });
    let mut f = std::fs::File::create(&out_path).context("创建输出文件失败")?;
    f.write_all(md.as_bytes())?;
    eprintln!(
        "完成：{} 页 → {}（共 {} 字符，{:.1}s）",
        total,
        out_path.display(),
        md.chars().count(),
        t0.elapsed().as_secs_f32()
    );

    // 5. 可选脱敏
    if args.redact {
        if is_image(&args.input) {
            eprintln!("--redact 仅对 PDF 输入生效，已跳过");
        } else {
            let p = redact_pdf(&args.input)?;
            eprintln!("脱敏副本：{}", p.display());
        }
    }

    if let Some(dir) = cleanup_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
    Ok(())
}

/// 多页 PDF → PNG 序列（走 pdftoppm，无 -singlefile，零填充保证排序）
fn pdf_to_pngs(pdf: &Path, dpi: u32) -> Result<PathBuf> {
    if which_pdftoppm().is_none() {
        bail!(
            "处理 PDF 需要 poppler（pdftoppm）。安装：\n  macOS:  brew install poppler\n  Ubuntu: sudo apt install poppler-utils"
        );
    }
    let dir = tempdir()?;
    let status = Command::new("pdftoppm")
        .args([
            "-r",
            &dpi.to_string(),
            "-png",
            "-cropbox",
        ])
        .arg(pdf)
        .arg(dir.join("page"))
        .status()
        .context("启动 pdftoppm 失败")?;
    if !status.success() {
        bail!("pdftoppm 退出码 {:?}", status.code());
    }
    Ok(dir)
}
