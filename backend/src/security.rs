use anyhow::{bail, Result};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

/// Validate document ID to prevent path traversal, command injection, and directory leakage.
/// Only allows alphanumeric characters, hyphens, and underscores between 1 and 64 characters.
pub fn validate_doc_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 {
        bail!("Invalid document ID length (must be between 1 and 64 characters)");
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("Invalid document ID characters (only alphanumeric, '-' and '_' allowed)");
    }
    Ok(())
}

/// Sanitize filename by stripping directory path components, null bytes,
/// control characters, and limiting length.
pub fn sanitize_filename(filename: &str) -> String {
    let base = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");

    let mut clean: String = base
        .chars()
        .filter(|&c| c != '\0' && c != '/' && c != '\\' && !c.is_control())
        .collect();

    // Prevent "." or ".." or empty filename
    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "document.pdf".to_string();
    }

    // Limit length to 255 chars
    if clean.len() > 255 {
        let ext = Path::new(&clean)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("pdf");
        clean = format!("doc_{}.{}", &clean[..32.min(clean.len())], ext);
    }

    clean
}

/// Safely truncate string by Unicode scalar count without slicing inside multi-byte codepoints
pub fn safe_truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Escape XML characters to prevent XML injection & stored XSS in XFDF
pub fn escape_xml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape PDF literal string contents (BT ... (str) Tj ... ET)
/// Prevents PDF stream injection, postscript breakout, and syntax breakage.
pub fn escape_pdf_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Check if an IP address is private, loopback, link-local, or cloud metadata
pub fn is_private_or_restricted_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 0.0.0.0/8 (Current network)
            octets[0] == 0
                // 10.0.0.0/8 (Private)
                || octets[0] == 10
                // 127.0.0.0/8 (Loopback)
                || octets[0] == 127
                // 169.254.0.0/16 (Link-local & AWS/GCP/Azure cloud metadata)
                || (octets[0] == 169 && octets[1] == 254)
                // 172.16.0.0/12 (Private)
                || (octets[0] == 172 && (octets[1] >= 16 && octets[1] <= 31))
                // 192.168.0.0/16 (Private)
                || (octets[0] == 192 && octets[1] == 168)
                // 224.0.0.0/4 (Multicast)
                || octets[0] >= 224
                // 255.255.255.255 (Broadcast)
                || ipv4.is_broadcast()
        }
        IpAddr::V6(ipv6) => {
            if let Some(mapped_v4) = ipv6.to_ipv4() {
                return is_private_or_restricted_ip(IpAddr::V4(mapped_v4));
            }
            // Loopback ::1, unspecified ::
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                // Unique Local Addresses (fc00::/7)
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                // Link Local Unicast (fe80::/10)
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Validate a remote URL to prevent SSRF against internal services, cloud metadata, and loopback.
pub fn validate_ssrf_url(url_str: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(url_str)?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("Protocole non autorisé : seuls http et https sont acceptés");
    }

    let host_str = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL invalide : hôte manquant"))?;

    // Block localhost names explicitly
    let host_lower = host_str.to_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        bail!("Accès refusé : l'hôte cible résout vers une adresse locale");
    }

    // Resolve hostname to IP addresses and verify none are private/internal
    let port = parsed.port_or_known_default().unwrap_or(80);
    let socket_addr_str = format!("{}:{}", host_str, port);
    match socket_addr_str.to_socket_addrs() {
        Ok(addrs) => {
            for addr in addrs {
                if is_private_or_restricted_ip(addr.ip()) {
                    bail!(
                        "Accès refusé par la politique SSRF : l'adresse IP {} est privée, locale ou restreinte",
                        addr.ip()
                    );
                }
            }
        }
        Err(e) => {
            bail!("Impossible de résoudre le nom d'hôte '{}': {}", host_str, e);
        }
    }

    Ok(parsed)
}

/// Check and canonicalize safe child path inside a base directory to prevent Path Traversal
pub fn safe_join_path(base_dir: &Path, user_rel_path: &str) -> Result<PathBuf> {
    if user_rel_path.contains("..") || user_rel_path.contains('\0') {
        bail!("Tentative de traversée de chemin détectée");
    }

    let clean = user_rel_path.trim_start_matches(|c| c == '/' || c == '\\');
    let target = base_dir.join(clean);

    // If target exists, verify canonical path
    if target.exists() {
        let can_base = base_dir.canonicalize()?;
        let can_target = target.canonicalize()?;
        if !can_target.starts_with(&can_base) {
            bail!("Accès refusé : le chemin est en dehors du répertoire racine");
        }
        Ok(can_target)
    } else {
        // Parent must be within base_dir
        if let Some(parent) = target.parent() {
            if parent.exists() {
                let can_base = base_dir.canonicalize()?;
                let can_parent = parent.canonicalize()?;
                if !can_parent.starts_with(&can_base) {
                    bail!("Accès refusé : le chemin est en dehors du répertoire racine");
                }
            }
        }
        Ok(target)
    }
}
