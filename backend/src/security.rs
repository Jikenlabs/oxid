use anyhow::{bail, Result};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

/// Valide l'identifiant de document pour empêcher la traversée de chemin, l'injection de commandes et la fuite d'arborescence.
/// Autorise uniquement les caractères alphanumériques, tirets et underscores d'une longueur de 1 à 64 caractères.
pub fn validate_doc_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 {
        bail!("Longueur d'identifiant de document invalide (doit être comprise entre 1 et 64 caractères)");
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("Caractères d'identifiant de document invalides (seuls les caractères alphanumériques, '-' et '_' sont autorisés)");
    }
    Ok(())
}

/// Assainit le nom de fichier en supprimant les segments de chemin, les octets nuls,
/// les caractères de contrôle et en tronquant la longueur totale.
pub fn sanitize_filename(filename: &str) -> String {
    let base = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");

    let mut clean: String = base
        .chars()
        .filter(|&c| c != '\0' && c != '/' && c != '\\' && !c.is_control())
        .collect();

    // Empêche les noms vides, "." ou ".."
    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "document.pdf".to_string();
    }

    // Limite la taille maximale du nom à 255 caractères
    if clean.len() > 255 {
        let ext = Path::new(&clean)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("pdf");
        clean = format!("doc_{}.{}", &clean[..32.min(clean.len())], ext);
    }

    clean
}

/// Tronque une chaîne de caractères de manière sécurisée en comptant les scalaires Unicode sans découper un point de code multi-octets
pub fn safe_truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Échappe les entités XML pour prévenir l'injection XML et les attaques XSS stockées dans les formulaires XFDF
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

/// Échappe le contenu des chaînes littérales PDF (opérateurs BT ... (str) Tj ... ET)
/// Empêche l'injection de flux PostScript/PDF et la corruption syntaxique.
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

/// Vérifie si une adresse IP est privée, de bouclage local, de liaison locale ou réservée aux métadonnées cloud
pub fn is_private_or_restricted_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 0.0.0.0/8 (Réseau local actuel)
            octets[0] == 0
                // 10.0.0.0/8 (Réseau privé classe A)
                || octets[0] == 10
                // 127.0.0.0/8 (Bouclage local loopback)
                || octets[0] == 127
                // 169.254.0.0/16 (Liaison locale link-local & métadonnées cloud AWS/GCP/Azure)
                || (octets[0] == 169 && octets[1] == 254)
                // 172.16.0.0/12 (Réseau privé classe B)
                || (octets[0] == 172 && (octets[1] >= 16 && octets[1] <= 31))
                // 192.168.0.0/16 (Réseau privé classe C)
                || (octets[0] == 192 && octets[1] == 168)
                // 224.0.0.0/4 (Adresses multicast)
                || octets[0] >= 224
                // 255.255.255.255 (Diffusion broadcast)
                || ipv4.is_broadcast()
        }
        IpAddr::V6(ipv6) => {
            if let Some(mapped_v4) = ipv6.to_ipv4() {
                return is_private_or_restricted_ip(IpAddr::V4(mapped_v4));
            }
            // Loopback ::1, non spécifiée ::
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                // Adresses locales uniques ULA (fc00::/7)
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                // Adresses link-local unicast (fe80::/10)
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Valide une URL distante pour prévenir les attaques SSRF vers les services internes, métadonnées cloud et boucle locale.
pub fn validate_ssrf_url(url_str: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(url_str)?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("Protocole non autorisé : seuls http et https sont acceptés");
    }

    let host_str = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL invalide : hôte manquant"))?;

    // Blocage explicite des noms de domaines locaux
    let host_lower = host_str.to_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        bail!("Accès refusé : l'hôte cible résout vers une adresse locale");
    }

    // Résolution DNS de l'hôte et vérification de l'absence d'adresses privées/internes
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

/// Vérifie et canonise un chemin relatif sécurisé dans un répertoire de base (protection Path Traversal)
pub fn safe_join_path(base_dir: &Path, user_rel_path: &str) -> Result<PathBuf> {
    if user_rel_path.contains("..") || user_rel_path.contains('\0') {
        bail!("Tentative de traversée de chemin détectée");
    }

    let clean = user_rel_path.trim_start_matches(|c| c == '/' || c == '\\');
    let target = base_dir.join(clean);

    // Si la cible existe, validation de la canonicité stricte
    if target.exists() {
        let can_base = base_dir.canonicalize()?;
        let can_target = target.canonicalize()?;
        if !can_target.starts_with(&can_base) {
            bail!("Accès refusé : le chemin est en dehors du répertoire racine");
        }
        Ok(can_target)
    } else {
        // Le répertoire parent doit impérativement résider dans base_dir
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
