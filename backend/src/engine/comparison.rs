use crate::engine::pdf::PdfEngine;
use crate::models::{DiffOp, TextDiffResult, TextDiffToken};
use anyhow::{bail, Result};
use image::{GenericImageView, ImageBuffer, Rgba};
use std::path::Path;

pub struct ComparisonEngine;

pub struct DiffResult {
    pub diff_ratio: f64,
    pub has_differences: bool,
    pub diff_image_png: Vec<u8>,
}

impl ComparisonEngine {
    pub fn compare_pages(
        path_a: &Path,
        path_b: &Path,
        page: usize,
        dpi: u32,
    ) -> Result<DiffResult> {
        let res_a = PdfEngine::render_page(path_a, page, dpi);
        let res_b = PdfEngine::render_page(path_b, page, dpi);

        if res_a.is_err() && res_b.is_err() {
            bail!("Page {} out of range in both documents", page);
        }

        let (img_a, img_b) = match (res_a, res_b) {
            (Ok(bytes_a), Ok(bytes_b)) => {
                let a = image::load_from_memory(&bytes_a)?;
                let b = image::load_from_memory(&bytes_b)?;
                (a, b)
            }
            (Ok(bytes_a), Err(_)) => {
                // Page exists in A but removed in B -> B is pure white
                let a = image::load_from_memory(&bytes_a)?;
                let (w, h) = a.dimensions();
                let b = image::DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([255, 255, 255, 255])));
                (a, b)
            }
            (Err(_), Ok(bytes_b)) => {
                // Page exists in B but added in B (didn't exist in A) -> A is pure white
                let b = image::load_from_memory(&bytes_b)?;
                let (w, h) = b.dimensions();
                let a = image::DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([255, 255, 255, 255])));
                (a, b)
            }
            (Err(e), _) => return Err(e),
        };


        let (w_a, h_a) = img_a.dimensions();
        let (w_b, h_b) = img_b.dimensions();

        let width = w_a.max(w_b);
        let height = h_a.max(h_b);

        let mut diff_img = ImageBuffer::new(width, height);
        let mut different_pixels = 0u64;
        let total_pixels = (width as u64) * (height as u64);

        for y in 0..height {
            for x in 0..width {
                let p_a = if x < w_a && y < h_a {
                    img_a.get_pixel(x, y)
                } else {
                    Rgba([255, 255, 255, 255])
                };

                let p_b = if x < w_b && y < h_b {
                    img_b.get_pixel(x, y)
                } else {
                    Rgba([255, 255, 255, 255])
                };

                let lum_a = ((p_a[0] as u32 + p_a[1] as u32 + p_a[2] as u32) / 3) as u8;
                let lum_b = ((p_b[0] as u32 + p_b[1] as u32 + p_b[2] as u32) / 3) as u8;

                let diff = (lum_a as i32 - lum_b as i32).abs();

                if diff > 15 {
                    different_pixels += 1;
                    if lum_a < lum_b {
                        // Present in A but removed in B -> RED
                        diff_img.put_pixel(x, y, Rgba([230, 40, 40, 255]));
                    } else {
                        // Added in B -> GREEN
                        diff_img.put_pixel(x, y, Rgba([40, 190, 60, 255]));
                    }
                } else {
                    // Unchanged: faded gray
                    let gray = ((lum_a as u16 + 200) / 2) as u8;
                    diff_img.put_pixel(x, y, Rgba([gray, gray, gray, 255]));
                }
            }
        }

        let diff_ratio = if total_pixels > 0 {
            (different_pixels as f64) / (total_pixels as f64)
        } else {
            0.0
        };

        let mut png_bytes = std::io::Cursor::new(Vec::new());
        diff_img.write_to(&mut png_bytes, image::ImageFormat::Png)?;

        Ok(DiffResult {
            diff_ratio,
            has_differences: different_pixels > 50,
            diff_image_png: png_bytes.into_inner(),
        })
    }

    pub fn compare_text_pages(
        path_a: &Path,
        path_b: &Path,
        page: usize,
    ) -> Result<TextDiffResult> {
        let text_a = PdfEngine::get_page_plain_text(path_a, page).unwrap_or_default();
        let text_b = PdfEngine::get_page_plain_text(path_b, page).unwrap_or_default();

        let tokens_a = tokenize_text(&text_a);
        let tokens_b = tokenize_text(&text_b);

        let tokens = compute_tokens_diff(&tokens_a, &tokens_b);

        let mut additions_count = 0;
        let mut deletions_count = 0;
        let mut unchanged_count = 0;

        for t in &tokens {
            match t.op {
                DiffOp::Insert => additions_count += tokenize_text(&t.text).len(),
                DiffOp::Delete => deletions_count += tokenize_text(&t.text).len(),
                DiffOp::Equal => unchanged_count += tokenize_text(&t.text).len(),
            }
        }

        let total = additions_count + deletions_count + unchanged_count;
        let diff_ratio = if total > 0 {
            (additions_count + deletions_count) as f64 / total as f64
        } else {
            0.0
        };

        Ok(TextDiffResult {
            page_number: page,
            additions_count,
            deletions_count,
            unchanged_count,
            diff_ratio,
            tokens,
        })
    }
}

fn tokenize_text(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut was_ws = false;

    for ch in s.chars() {
        if ch.is_whitespace() {
            if !was_ws && !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            was_ws = true;
            cur.push(ch);
        } else if ch.is_alphanumeric() {
            if was_ws && !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            was_ws = false;
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            tokens.push(ch.to_string());
            was_ws = false;
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn compute_tokens_diff(tokens_a: &[String], tokens_b: &[String]) -> Vec<TextDiffToken> {
    let m = tokens_a.len();
    let n = tokens_b.len();

    if m == 0 && n == 0 {
        return Vec::new();
    }
    if m == 0 {
        return vec![TextDiffToken {
            op: DiffOp::Insert,
            text: tokens_b.concat(),
        }];
    }
    if n == 0 {
        return vec![TextDiffToken {
            op: DiffOp::Delete,
            text: tokens_a.concat(),
        }];
    }

    let (t_a, t_b) = if m > 3000 || n > 3000 {
        (&tokens_a[..m.min(3000)], &tokens_b[..n.min(3000)])
    } else {
        (tokens_a, tokens_b)
    };

    let len_a = t_a.len();
    let len_b = t_b.len();
    let stride = len_b + 1;
    let mut dp = vec![0u32; (len_a + 1) * stride];

    for i in 0..len_a {
        for j in 0..len_b {
            if t_a[i] == t_b[j] {
                dp[(i + 1) * stride + (j + 1)] = dp[i * stride + j] + 1;
            } else {
                let left = dp[(i + 1) * stride + j];
                let up = dp[i * stride + (j + 1)];
                dp[(i + 1) * stride + (j + 1)] = left.max(up);
            }
        }
    }

    let mut raw_diff = Vec::new();
    let mut i = len_a;
    let mut j = len_b;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && t_a[i - 1] == t_b[j - 1] {
            raw_diff.push(TextDiffToken {
                op: DiffOp::Equal,
                text: t_a[i - 1].clone(),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i * stride + (j - 1)] >= dp[(i - 1) * stride + j]) {
            raw_diff.push(TextDiffToken {
                op: DiffOp::Insert,
                text: t_b[j - 1].clone(),
            });
            j -= 1;
        } else if i > 0 {
            raw_diff.push(TextDiffToken {
                op: DiffOp::Delete,
                text: t_a[i - 1].clone(),
            });
            i -= 1;
        }
    }
    raw_diff.reverse();

    let mut tokens: Vec<TextDiffToken> = Vec::new();
    for item in raw_diff {
        if let Some(last) = tokens.last_mut() {
            if last.op == item.op {
                last.text.push_str(&item.text);
                continue;
            }
        }
        tokens.push(item);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenization_and_diff() {
        let text_a = "Le contrat est conclu le 12/01/2025 pour un montant de 10 000 €.";
        let text_b = "Le contrat est conclu le 15/02/2026 pour un montant de 15 000 €.";

        let tokens_a = tokenize_text(text_a);
        let tokens_b = tokenize_text(text_b);

        let diff = compute_tokens_diff(&tokens_a, &tokens_b);

        println!("DIFF TOKENS: {:#?}", diff);

        let has_insert = diff.iter().any(|t| t.op == DiffOp::Insert);
        let has_delete = diff.iter().any(|t| t.op == DiffOp::Delete);
        let has_equal = diff.iter().any(|t| t.op == DiffOp::Equal && t.text.contains("Le contrat est conclu"));

        assert!(has_insert, "Diff should contain inserted date");
        assert!(has_delete, "Diff should contain deleted date");
        assert!(has_equal, "Diff should contain unchanged common text");
    }

    #[test]
    fn test_identical_text_diff() {
        let text = "Ceci est un document strictement identique.";
        let tokens = tokenize_text(text);
        let diff = compute_tokens_diff(&tokens, &tokens);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].op, DiffOp::Equal);
        assert_eq!(diff[0].text, text);
    }
}
