use crate::engine::pdf::PdfEngine;
use crate::models::{
    GlobalSearchResponse, PiiCategory, PiiItem, PiiScanResponse, SearchMatchItem, TextSpan,
};
use anyhow::Result;
use std::path::Path;

pub struct PiiEngine;

impl PiiEngine {
    /// Analyse l'ensemble des pages d'un document pour détecter des données sensibles (PII / RGPD)
    pub fn scan_document(path: &Path, page_count: usize) -> Result<PiiScanResponse> {
        let mut items = Vec::new();

        for page_num in 1..=page_count {
            let page_text = match PdfEngine::get_page_text(path, page_num) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let page_items = Self::scan_page_spans(page_num, &page_text.spans);
            items.extend(page_items);
        }

        Ok(PiiScanResponse {
            total_found: items.len(),
            items,
        })
    }

    /// Analyse les segments de texte d'une page pour repérer des motifs sensibles
    pub fn scan_page_spans(page_num: usize, spans: &[TextSpan]) -> Vec<PiiItem> {
        let mut items = Vec::new();

        // 1. Détection sur un segment unique (adresses e-mail, NIR sans espace, téléphones, etc.)
        for span in spans {
            let text = span.text.trim();
            if text.is_empty() {
                continue;
            }

            // Vérification adresse électronique
            if Self::is_valid_email(text) {
                items.push(PiiItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    category: PiiCategory::Email,
                    matched_text: text.to_string(),
                    masked_preview: Self::mask_email(text),
                    page_number: page_num,
                    x: span.x,
                    y: span.y,
                    width: span.width,
                    height: span.height,
                });
            }

            // Vérification IBAN sur segment unique
            let iban_clean = text.replace(' ', "").to_uppercase();
            if iban_clean.len() >= 15 && iban_clean.len() <= 34 && Self::is_valid_iban(&iban_clean) {
                items.push(PiiItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    category: PiiCategory::Iban,
                    matched_text: text.to_string(),
                    masked_preview: Self::mask_iban(&iban_clean),
                    page_number: page_num,
                    x: span.x,
                    y: span.y,
                    width: span.width,
                    height: span.height,
                });
            }


            // Vérification numéro de sécurité sociale (NIR à 13 ou 15 chiffres)
            let digits_only: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
            if (digits_only.len() == 13 || digits_only.len() == 15) && Self::is_valid_french_nir(&digits_only) {
                items.push(PiiItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    category: PiiCategory::SocialSecurity,
                    matched_text: text.to_string(),
                    masked_preview: Self::mask_digits(&digits_only),
                    page_number: page_num,
                    x: span.x,
                    y: span.y,
                    width: span.width,
                    height: span.height,
                });
            }

            // Vérification carte bancaire (segment unique)
            if (digits_only.len() >= 13 && digits_only.len() <= 19) && Self::is_valid_luhn(&digits_only) {
                items.push(PiiItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    category: PiiCategory::CreditCard,
                    matched_text: text.to_string(),
                    masked_preview: Self::mask_digits(&digits_only),
                    page_number: page_num,
                    x: span.x,
                    y: span.y,
                    width: span.width,
                    height: span.height,
                });
            }

            // Vérification numéro de téléphone (ex. +33..., 06..., 07..., 10 chiffres)
            if Self::is_valid_phone(text) {
                items.push(PiiItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    category: PiiCategory::Phone,
                    matched_text: text.to_string(),
                    masked_preview: Self::mask_phone(text),
                    page_number: page_num,
                    x: span.x,
                    y: span.y,
                    width: span.width,
                    height: span.height,
                });
            }
        }

        // 2. Fenêtre glissante horizontale sur plusieurs segments consécutifs (IBAN et cartes bancaires avec espaces)
        let n = spans.len();
        for window_size in 2..=8 {
            if window_size > n {
                break;
            }
            for i in 0..=(n - window_size) {
                let slice = &spans[i..i + window_size];
                // Vérification de l'alignement sur une même ligne horizontale
                let first_y = slice[0].y;
                let same_line = slice.iter().all(|s| (s.y - first_y).abs() < 5.0);
                if !same_line {
                    continue;
                }

                let combined_raw: String = slice.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                let combined_clean: String = slice.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("");

                // Vérification IBAN multi-segments
                let iban_clean: String = combined_clean.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
                if iban_clean == combined_clean.to_uppercase() && iban_clean.len() >= 15 && iban_clean.len() <= 34 && Self::is_valid_iban(&iban_clean) {
                    let min_x = slice.iter().map(|s| s.x).fold(f64::INFINITY, f64::min);
                    let max_x = slice.iter().map(|s| s.x + s.width).fold(f64::NEG_INFINITY, f64::max);
                    let min_y = slice.iter().map(|s| s.y).fold(f64::INFINITY, f64::min);
                    let max_y = slice.iter().map(|s| s.y + s.height).fold(f64::NEG_INFINITY, f64::max);

                    // Évite l'insertion de sous-intervalles doublons
                    if !items.iter().any(|it| it.matched_text.replace(' ', "") == iban_clean) {
                        items.push(PiiItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            category: PiiCategory::Iban,
                            matched_text: combined_raw.clone(),
                            masked_preview: Self::mask_iban(&iban_clean),
                            page_number: page_num,
                            x: min_x,
                            y: min_y,
                            width: max_x - min_x,
                            height: max_y - min_y,
                        });
                    }
                }

                // Vérification carte bancaire avec espaces
                let digits: String = combined_clean.chars().filter(|c| c.is_ascii_digit()).collect();
                if (digits.len() >= 13 && digits.len() <= 19) && Self::is_valid_luhn(&digits) {
                    let min_x = slice.iter().map(|s| s.x).fold(f64::INFINITY, f64::min);
                    let max_x = slice.iter().map(|s| s.x + s.width).fold(f64::NEG_INFINITY, f64::max);
                    let min_y = slice.iter().map(|s| s.y).fold(f64::INFINITY, f64::min);
                    let max_y = slice.iter().map(|s| s.y + s.height).fold(f64::NEG_INFINITY, f64::max);

                    if !items.iter().any(|it| it.matched_text.replace(' ', "") == digits) {
                        items.push(PiiItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            category: PiiCategory::CreditCard,
                            matched_text: combined_raw.clone(),
                            masked_preview: Self::mask_digits(&digits),
                            page_number: page_num,
                            x: min_x,
                            y: min_y,
                            width: max_x - min_x,
                            height: max_y - min_y,
                        });
                    }
                }

                // Vérification NIR français avec espaces (ex. 1 85 12 75 108 042 12)
                if (digits.len() == 13 || digits.len() == 15) && Self::is_valid_french_nir(&digits) {
                    let min_x = slice.iter().map(|s| s.x).fold(f64::INFINITY, f64::min);
                    let max_x = slice.iter().map(|s| s.x + s.width).fold(f64::NEG_INFINITY, f64::max);
                    let min_y = slice.iter().map(|s| s.y).fold(f64::INFINITY, f64::min);
                    let max_y = slice.iter().map(|s| s.y + s.height).fold(f64::NEG_INFINITY, f64::max);

                    if !items.iter().any(|it| it.matched_text.replace(' ', "") == digits) {
                        items.push(PiiItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            category: PiiCategory::SocialSecurity,
                            matched_text: combined_raw.clone(),
                            masked_preview: Self::mask_digits(&digits),
                            page_number: page_num,
                            x: min_x,
                            y: min_y,
                            width: max_x - min_x,
                            height: max_y - min_y,
                        });
                    }
                }
            }
        }

        items
    }

    /// Recherche plein texte dans l'intégralité du document sur toutes les pages
    pub fn search_document(
        path: &Path,
        page_count: usize,
        query: &str,
    ) -> Result<GlobalSearchResponse> {
        let clean_q = query.trim();
        if clean_q.is_empty() {
            return Ok(GlobalSearchResponse {
                query: query.to_string(),
                total_matches: 0,
                matches: vec![],
            });
        }

        let lower_q = clean_q.to_lowercase();
        let mut matches = Vec::new();

        for page_num in 1..=page_count {
            let page_text = match PdfEngine::get_page_text(path, page_num) {
                Ok(t) => t,
                Err(_) => continue,
            };

            for span in &page_text.spans {
                if span.text.to_lowercase().contains(&lower_q) {
                    matches.push(SearchMatchItem {
                        page_number: page_num,
                        text: span.text.clone(),
                        x: span.x,
                        y: span.y,
                        width: span.width,
                        height: span.height,
                    });
                }
            }
        }

        Ok(GlobalSearchResponse {
            query: query.to_string(),
            total_matches: matches.len(),
            matches,
        })
    }

    // --- Algorithmes de validation et sommes de contrôle (Checksums) ---

    /// Algorithme de Luhn pour la validation des numéros de carte bancaire.
    pub fn is_valid_luhn(number: &str) -> bool {
        if number.len() < 13 || number.len() > 19 {
            return false;
        }
        let mut sum = 0;
        let mut double = false;

        for ch in number.chars().rev() {
            if let Some(digit) = ch.to_digit(10) {
                if double {
                    let d = digit * 2;
                    sum += if d > 9 { d - 9 } else { d };
                } else {
                    sum += digit;
                }
                double = !double;
            } else {
                return false;
            }
        }

        sum % 10 == 0
    }

    /// Vérification de la clé du numéro de sécurité sociale français (NIR) : Clé = 97 - (NIR % 97)
    pub fn is_valid_french_nir(nir_str: &str) -> bool {
        let first_char = match nir_str.chars().next() {
            Some(c) => c,
            None => return false,
        };
        // 1 = homme, 2 = femme (standard)
        if first_char != '1' && first_char != '2' {
            return false;
        }

        if nir_str.len() == 15 {
            let base = &nir_str[0..13];
            let key_str = &nir_str[13..15];
            if let (Ok(base_num), Ok(key_num)) = (base.parse::<u64>(), key_str.parse::<u64>()) {
                let expected_key = 97 - (base_num % 97);
                return key_num == expected_key;
            }
        } else if nir_str.len() == 13 {
            // Vérifie la structure de base si la clé n'est pas fournie
            return nir_str.chars().all(|c| c.is_ascii_digit());
        }

        false
    }

    /// Vérification ISO 7064 Modulo 97-10 pour la validation IBAN.
    pub fn is_valid_iban(iban: &str) -> bool {
        let clean: String = iban.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if clean.len() < 15 || clean.len() > 34 {
            return false;
        }

        // Code pays (2 lettres) + 2 chiffres de contrôle
        let chars: Vec<char> = clean.chars().collect();
        if !chars[0].is_ascii_alphabetic() || !chars[1].is_ascii_alphabetic() {
            return false;
        }
        if !chars[2].is_ascii_digit() || !chars[3].is_ascii_digit() {
            return false;
        }

        // Déplace les 4 premiers caractères à la fin
        let rearranged = format!("{}{}", &clean[4..], &clean[0..4]);


        // Convertit les lettres en nombres (A=10, B=11, ..., Z=35)
        let mut numeric_string = String::with_capacity(rearranged.len() * 2);
        for c in rearranged.chars() {
            if c.is_ascii_digit() {
                numeric_string.push(c);
            } else if c.is_ascii_uppercase() {
                let val = (c as u32) - ('A' as u32) + 10;
                numeric_string.push_str(&val.to_string());
            } else {
                return false;
            }
        }

        // Calcule le modulo 97 par morceaux (pour éviter les dépassements d'entier)
        let mut remainder = 0;
        for chunk in numeric_string.as_bytes().chunks(7) {
            let chunk_str = match std::str::from_utf8(chunk) {
                Ok(s) => s,
                Err(_) => return false,
            };
            let combined = format!("{}{}", remainder, chunk_str);
            if let Ok(num) = combined.parse::<u64>() {
                remainder = num % 97;
            } else {
                return false;
            }
        }

        remainder == 1
    }

    /// Validation de la syntaxe d'adresse email.
    pub fn is_valid_email(text: &str) -> bool {
        if !text.contains('@') || !text.contains('.') {
            return false;
        }
        let parts: Vec<&str> = text.split('@').collect();
        if parts.len() != 2 {
            return false;
        }
        let (user, domain) = (parts[0], parts[1]);
        if user.is_empty() || domain.is_empty() {
            return false;
        }
        if !domain.contains('.') {
            return false;
        }
        let ext = domain.rsplit('.').next().unwrap_or("");
        ext.len() >= 2 && ext.chars().all(|c| c.is_ascii_alphabetic())
    }

    /// Détection et validation des numéros de téléphone.
    pub fn is_valid_phone(text: &str) -> bool {
        let clean = text.replace([' ', '.', '-', '(', ')'], "");
        if clean.starts_with('+') && clean.len() >= 10 && clean.len() <= 15 {
            return clean[1..].chars().all(|c| c.is_ascii_digit());
        }
        if (clean.starts_with("06") || clean.starts_with("07") || clean.starts_with("01") || clean.starts_with("02") || clean.starts_with("03") || clean.starts_with("04") || clean.starts_with("05") || clean.starts_with("09")) && clean.len() == 10 {
            return clean.chars().all(|c| c.is_ascii_digit());
        }
        false
    }

    // --- Fonctions utilitaires de masquage pour l'affichage UI ---

    pub fn mask_email(email: &str) -> String {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return "******@***.com".to_string();
        }
        let user = parts[0];
        let domain = parts[1];
        let user_chars: Vec<char> = user.chars().collect();
        let masked_user = if user_chars.len() <= 2 {
            "**".to_string()
        } else {
            let first = user_chars.first().copied().unwrap_or('*');
            let last = user_chars.last().copied().unwrap_or('*');
            format!("{}***{}", first, last)
        };
        format!("{}@{}", masked_user, domain)
    }

    pub fn mask_digits(num: &str) -> String {
        let chars: Vec<char> = num.chars().collect();
        if chars.len() <= 4 {
            return "****".to_string();
        }
        let visible_tail: String = chars[chars.len() - 4..].iter().collect();
        format!("•••• •••• •••• {}", visible_tail)
    }

    pub fn mask_iban(iban: &str) -> String {
        let chars: Vec<char> = iban.chars().collect();
        if chars.len() <= 8 {
            return "IBAN **********".to_string();
        }
        let head: String = chars[..4].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{} •••• •••• {}", head, tail)
    }

    pub fn mask_phone(phone: &str) -> String {
        let chars: Vec<char> = phone.chars().collect();
        if chars.len() <= 4 {
            return "•• •• •• ••".to_string();
        }
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("+•• • •• •• {}", tail)
    }
}
