# 🔒 Architecture de Sécurité & Conformité Réglementaire — Oxid

Ce document détaille la politique de sécurité, les mécanismes de protection contre les vulnérabilités et la conformité aux exigences réglementaires (**RGPD, Secret Médical, ISO 19444-1**) de la plateforme **Oxid**.

---

## 1. Modèle de Sécurité en Profondeur (Defense-in-Depth)

Oxid adopte les principes du **Zero-Trust** et de la **sécurité par conception (Security by Design)** :

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. PÉRIMÈTRE RÉSEAU & TRANSPORT                                        │
│    - Chiffrement TLS 1.3 obligatoire de bout en bout                   │
│    - En-têtes HTTP de sécurité stricts (HSTS, CSP, X-Frame-Options)     │
│    - Contrôle granulaire CORS (Cross-Origin Resource Sharing)           │
└────────────────────────────────────┬────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ 2. CONTRÔLE D'ACCÈS & CONTEXTE DE SÉCURITÉ                              │
│    - Jeton d'autorisation (Bearer JWT RFC 7519 ou Ticket GED CMIS)     │
│    - Matrice RBAC (Lecture seule, Annotation, Biffure, Signature)       │
│    - Traçabilité de l'identité de l'opérateur (Audit Trail)             │
└────────────────────────────────────┬────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ 3. COUCHE D'ISOLATION APPLICATIVE (RUST ENGINE)                        │
│    - Sûreté Mémoire native (Zéro Buffer Overflow, Zéro Use-After-Free)  │
│    - Protection contre les injections XML (Désactivation XXE/DTD)       │
│    - Isolation des fichiers temporaires (Sandboxing OS)                 │
└────────────────────────────────────┬────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. TRAITEMENTS SÉCURISÉS DU CONTENU DOCUMENTAIRE                        │
│    - Biffure permanente (Burn-in physique irréversible du texte)        │
│    - Filigrane dynamique anti-fuite visuelle                            │
│    - Scellement cryptographique d'intégrité par empreinte SHA-256       │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Protection contre le Top 10 OWASP & Vulnérabilités Spécifiques

### 2.1 Sûreté Mémoire (*Memory Safety*) — L'atout déterminant de Rust
Contrairement aux moteurs C/C++ ou aux environnements Java exposés aux failles de désérialisation (ex: *Log4j, Spring4Shell*) :
- Le compilateur Rust garantit l'absence d'erreurs de pointeurs, de fuites mémoire incontrôlées et de dépassements de tampon (*Buffer Overflow*).
- Aucun ramasse-miettes (*Garbage Collector*) n'est exploitable pour des attaques par saturation mémoire (OOM DoS).

### 2.2 Traversal de Répertoire (*Path Traversal*)
Tous les identifiants de documents reçus sont strictement validés :
```rust
// Validation stricte : seuls les caractères alphanumériques et tirets sont admis
if !doc_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
    return Err((StatusCode::BAD_REQUEST, "Identifiant document invalide"));
}
// Canonicalisation obligatoire et vérification de confinement dans OXID_DATA_DIR
let safe_path = base_dir.join(&doc_id).canonicalize()?;
if !safe_path.starts_with(&base_dir) {
    return Err((StatusCode::FORBIDDEN, "Accès refusé hors du répertoire autorisé"));
}
```

### 2.3 Injection XML & Failles XXE (*XML External Entity*)
Le moteur d'annotations XFDF utilise un analyseur XML configuré en mode sécurisé strict :
- La résolution d'entités externes (DTD externes, entités `SYSTEM`) est **totalement désactivée**.
- Les attaques par bombe de décompression XML (*Billion Laughs attack*) sont bloquées par une limite de profondeur d'arbre stricte.

### 2.4 Déni de Service & Épuisement de Ressources (DoS)
- **Taille maximale d'upload bornée** : Limitation à 100 Mo par document (`DefaultBodyLimit::max`).
- **Cache L1 borné en mémoire** : Éviction LRU automatique lorsque la limite définie par `OXID_CACHE_MEMORY_MB` est atteinte.
- **Exécution isolée des rasterisations lourdes** : Déportées dans le pool de threads de calcul Tokio via `tokio::task::spawn_blocking` pour ne jamais bloquer le serveur HTTP.

---

## 3. Biffure Permanente (Burn-in Redaction) & Conformité RGPD

### 3.1 Problématique des Biffures Virtuelles
Dans de nombreuses visionneuses documentaires de mauvaise qualité, "biffer" consiste simplement à dessiner un rectangle noir visuel par-dessus le texte dans le navigateur. **Cette pratique est dangereuse** :
- Le texte reste présent dans les flux PDF internes (`/Contents`).
- N'importe quel utilisateur peut copier le texte sous le rectangle noir ou l'extraire avec un simple script.

### 3.2 Implémentation du Burn-in Physique dans Oxid
Le moteur `RedactionEngine` applique une destruction physique irréversible :
1. **Suppression des flux d'instructions d'affichage** : Les commandes PDF `Tj`, `TJ` et `'` contenant les caractères sous la zone biffée sont réécrites ou purgées de l'arbre du document.
2. **Découpage des images matricielles** : Si une image se trouve sous la zone biffée, les pixels correspondants sont physiquement réécrits avec des zéros.
3. **Application du cartouche légal opaque** : Un rectangle noir solide est gravé dans le flux de la page avec impression du motif légal au centre.
4. **Conformité RGPD** : Garantit le respect strict de l'**Article 17 du RGPD (Droit à l'effacement)** et des règles du **Secret Médical**.

---

## 4. Filigranes Dynamiques Anti-Fuite (*Watermarking*)

Pour empêcher la diffusion non autorisée (ex: fuite de documents confidentiels par capture d'écran ou téléchargement illicite) :
- Oxid permet d'injecter un **filigrane dynamique semi-transparent en diagonale (45°)**.
- Le texte du filigrane peut intégrer dynamiquement :
  - Le nom ou l'adresse email de l'utilisateur connecté.
  - L'adresse IP source et la date/heure exacte de consultation.
  - La mention de confidentialité (*"DOCUMENT CONFIDENTIEL — USAGE INTERNE EXCLUSIF"*).
- Le filigrane est fusionné directement dans les tuiles d'images servies : il est impossible de le masquer via l'inspecteur DOM du navigateur.

---

## 5. Signature Électronique & Scellement Cryptographique SHA-256

Lorsqu'un document est signé via le viewer :
1. **Tracé Visuel** : Le paraphe ou la signature manuscrite est vectorisée et apposée sur la page cible.
2. **Cartouche d'Audit Horodaté** : Un cartouche légal est généré, mentionnant le nom du signataire, l'identifiant de transaction et la date certifiée UTC.
3. **Empreinte SHA-256** : Le document PDF résultant fait l'objet d'un scellement avec calcul de l'empreinte cryptographique SHA-256 intégrée dans la réponse d'audit.

---

## 6. Scanner de Données Sensibles PII (RGPD / DSP2)

L'endpoint `/api/documents/:id/pii-scan` intègre des filtres de détection automatique des données à caractère personnel :
- **Numéros de Sécurité Sociale (NIR français)** : Format `1|2 yy mm dd ooo nnn cc`.
- **Coordonnées Bancaires (IBAN)** : Validation selon l'algorithme Modulo-97 (ISO 7064).
- **Numéros de Cartes Bancaires** : Détection avec validation par l'algorithme de Luhn.
- **Adresses Email & Numéros de téléphone internationaux**.

Ces éléments détectés peuvent être surlignés pour assister le juriste ou le DPO dans la préparation des zones de biffure.
