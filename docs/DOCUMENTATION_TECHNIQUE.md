# 📘 Documentation Technique Complète — Oxid

Ce document constitue la référence d'architecture, d'implémentation et de spécification technique pour la plateforme **Oxid**.

---

## 1. Vue d'Ensemble & Architecture Globale

Oxid est un moteur de visualisation et de conversion de documents haute performance conçu en **Rust**, reposant sur un modèle asynchrone non-bloquant (Tokio/Axum), économe en ressources et sécurisé par conception (*Memory-Safe*).

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           CLIENT / NAVIGATEUR                           │
│  Web Component <oxid-viewer> (Shadow DOM) | Iframe | SPA React/Vue/Ang  │
│  Rendu multi-couches : Raster GPU + Couche Texte SVG + Couche XFDF     │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ HTTP/1.1 - HTTP/2 / WebSockets
┌────────────────────────────────────▼────────────────────────────────────┐
│                    API GATEWAY & ROUTEUR AXUM (Rust)                    │
│      Compression Gzip | Sécurité CORS | Streaming HTTP Range 206        │
└───────┬────────────────────────────┬────────────────────────────┬───────┘
        │                            │                            │
┌───────▼──────────────┐   ┌─────────▼──────────────┐   ┌─────────▼──────────────┐
│  MOTEURS DE RENDU    │   │  CACHE DISTRIBUÉ MULTI │   │    CONNECTEURS GED     │
│  - PDF Engine        │   │  - L1 : Mémoire RAM    │   │  - CMIS 1.1 (OpenCMIS) │
│  - Office (Docx/Xlsx)│   │  - L2 : NVMe / Redis   │   │  - S3 / MinIO          │
│  - DICOM Médical     │   │  - Invalidation TTL    │   │  - Filesystem local    │
│  - CAO / DAO (DXF)   │   │                        │   │  - HTTP/HTTPS distant  │
│  - Vidéo / Multimédia│   │                        │   │                        │
└──────────────────────┘   └────────────────────────┘   └────────────────────────┘
```

---

## 2. Piles Technologiques

### Backend
- **Langage** : Rust (Edition 2021, compilé en profil `release` avec LTO et `panic = "abort"`).
- **Moteur Asynchrone** : Tokio 1.40 (multi-threads work-stealing runtime).
- **Framework Web & API** : Axum 0.7 + Tower HTTP (CORS, Trace, ServeFile, Compression Gzip).
- **Moteur Vectoriel & PDF** : `lopdf` (parsing/manipulation d'arborescences de dictionnaires PDF) + `pdftoppm` (rasterisation haute fidélité).
- **Imagerie & Médical** : Décodeur binaire DICOM natif (norme DICOM PS3.5), fenêtrage Hounsfield dynamique, `image` crate (PNG, JPEG turbo, WebP).
- **Multimédia** : Intégration `ffmpeg` / `ffprobe` pour poster frames et streaming RFC 7233.
- **Cache & Stockage** : Cache distribué compatible Valkey / Redis + Cache local multi-niveaux.

### Frontend
- **Langage** : TypeScript 5.5 (compilation ciblée ES2022).
- **Composant Web** : Web Component autonome (`<oxid-viewer>`) encapsulé avec styles scopés.
- **Worker Dédié** : Web Worker (`render-worker.ts`) pour la pré-extraction asynchrone et le décodage d'images en tâche de fond (Look-ahead prédictif).
- **Bundler** : Vite 5 (génération d'un bundle standard UMD + module ES).

---

## 3. Moteurs de Rendu & Pipelines de Conversion

### A. Moteur PDF & Annotations (`PdfEngine`, `AnnotationEngine`)
- **Lecture des structures** : Extraction des dictionnaires de pages (`MediaBox`, `CropBox`, `Rotate`).
- **Rendu JPEG Turbo** : Conversion en JPEG 85% à 120-150 DPI par défaut. Vitesse de traitement : **35 à 65 ms** par page contre 400 à 800 ms avec PDFBox/Java.
- **Couche Texte Invisible** : Génération d'une couche SVG / HTML de glyphes synchronisée avec les coordonnées absolues pour permettre la sélection, la copie et la recherche textuelle.
- **Standard XFDF** : Les annotations sont sérialisées et désérialisées en XML ISO 19444-1 (Highlight, StrikeOut, Underline, FreeText, Square, Circle, Line, Ink, Stamp).

### B. Moteur Bureautique Office (`OfficeConverter`)
- Supporte le mode hybride :
  1. **Natif** : Conversion directe et ultra-rapide des flux `.docx`, `.xlsx`, `.txt`, `.csv`.
  2. **Headless** : Délégation automatique vers LibreOffice headless pour les fichiers complexes comportant macros ou mises en page héritées (`.doc`, `.ppt`, `.vsd`).

### C. Moteur d'Imagerie Médicale DICOM (`DicomConverter`)
- **Norme** : Support des fichiers `.dcm` et `.dicom` (radiographies, scanners CT, IRM, angiographies).
- **Fenêtrage Hounsfield (HU)** : Calcul à la volée du contraste selon les paramètres :
  $$\text{Pixel}_{\text{visu}} = \text{clamp}\left( \frac{(\text{Pixel}_{\text{raw}} \times \text{Slope} + \text{Intercept}) - (\text{WC} - 0.5)}{\text{WW} - 1} + 0.5, 0, 1 \right) \times 255$$
- **Presets de tissus anatomiques** : Cerveau (WC: 40, WW: 80), Poumons (WC: -600, WW: 1500), Os (WC: 400, WW: 1800), Tissus mous (WC: 50, WW: 350).
- **Lecteur Ciné Angiographie** : Gestion multi-frames avec défilement fluide à 15 fps en fond noir (`#000000`).

### D. Moteur CAO / DAO (`CadConverter`)
- **Normes supportées** : AutoCAD DXF (R12 à 2018) et DWG.
- **Extraction des entités** : Lignes, Cercles, Arcs, Polylignes, Textes, Hachures.
- **Table des calques (Layers)** : Détection des noms de calques, des couleurs ACI et association par entité.
- **Filtrage dynamique** : L'API `/pages/:p/render?layers=L1,L2` permet de recalculer le plan vectoriel avec masquage physique immédiat des calques désactivés.

### E. Moteur Multimédia Vidéo & Audio
- **Formats** : MP4 (H.264 / AAC), WebM (VP8/VP9), MOV, AVI, OGG.
- **Streaming RFC 7233** : Support natif du header HTTP `Range: bytes=start-end` renvoyant le code `206 Partial Content`. Permet le saut immédiat (seeking) à n'importe quelle seconde du fichier sans télécharger la vidéo complète.
- **Aperçu automatique** : Génération transparente d'un poster JPEG à 1 seconde via `ffmpeg` pour la barre latérale des vignettes.

---

## 4. Spécification de l'API REST

### Base URL : `http://<host>:<port>/api`

#### Documents & Cycle de vie

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/documents/upload` | Envoi d'un fichier (multipart/form-data `file`). Retourne les métadonnées (`id`, `page_count`, `pages`). |
| `GET` | `/documents/:id` | Récupération des métadonnées du document. |
| `GET` | `/documents/:id/download` | Téléchargement du document d'origine ou avec filigrane (`?watermark=...`). |
| `GET` | `/documents/:id/video` | Streaming vidéo/audio avec support des requêtes HTTP Range `206 Partial Content`. |
| `GET` | `/documents/:id/content` | Alias de streaming universel du fichier brut. |

#### Rendu & Pages

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/documents/:id/pages/:p/render` | Rendu graphique d'une page. Paramètres : `dpi` (ex: 150), `format` (jpeg/png), `layers` (CAO), `wc`/`ww` (DICOM). |
| `GET` | `/documents/:id/pages/:p/thumbnail` | Rendu de la miniature basse résolution (optimisé pour barre latérale). |
| `GET` | `/documents/:id/pages/:p/text` | Extraction de la couche texte invisible pour sélection et recherche. |

#### Annotations & GED

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/documents/:id/annotations` | Export de toutes les annotations au format standard XFDF. |
| `POST` | `/documents/:id/annotations` | Enregistrement d'annotations XFDF avec synchronisation GED. |
| `GET` | `/documents/:id/collab` | Flux Server-Sent Events / WebSocket de collaboration temps réel. |
| `POST` | `/documents/:id/redact` | Application de biffures physiques irréversibles (burn-in vectoriel). |
| `POST` | `/documents/:id/sign` | Signature électronique visuelle et cryptographique (SHA-256). |

#### Formulaires & Données

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/documents/:id/forms` | Détection et lecture des champs de formulaire AcroForms. |
| `POST` | `/documents/:id/forms/fill` | Injection de données dans les champs de formulaire et sauvegarde. |
| `GET` | `/documents/:id/pii-scan` | Détection automatique des données sensibles (Email, IBAN, CB, SSN). |
| `GET` | `/documents/:id/search?q=...`| Recherche textuelle plein texte dans l'ensemble des pages. |

#### Métadonnées Spécialisées (CAO & Médical)

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/documents/:id/cad/layers` | Liste des calques CAO/DAO avec nom, couleur hex et nombre d'éléments. |
| `GET` | `/documents/:id/dicom/metadata` | Données patient, modalité (CT/MR/XA), institut et presets Hounsfield. |

#### Connecteurs & Exploitation

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/connectors/open` | Ouverture d'un document distant via `connector=cmis\|s3\|url` et `id=...`. |
| `GET` | `/health` | Bilan de santé du nœud (`status: ok`, `node_id`, `version`). |
| `GET` | `/metrics` | Métriques au format Prometheus (`cache_hits`, `http_requests_total`). |
| `GET` | `/cluster/status` | Taux de succès du cache L1 (RAM) et L2 (Disque/Redis). |

---

## 5. Stratégie de Cache Multi-Niveaux

Oxid implémente une stratégie de cache hiérarchique à 2 niveaux :

1. **Cache L1 (RAM)** :
   - Stockage en mémoire vive des tuiles et pages récemment demandées.
   - Accès en **< 1 milliseconde**.
   - Éviction LRU (Least Recently Used) paramétrable via `OXID_CACHE_MEMORY_MB`.
2. **Cache L2 (Disque NVMe ou Cluster Redis/Valkey)** :
   - Rendu persisté sous clé SHA-256 : `{doc_id}:{page}:{dpi}:{variant}:{layers}:{wc}:{ww}`.
   - Partagé entre tous les conteneurs d'un cluster Kubernetes.
   - Zéro recalcul si un utilisateur consulte une page déjà rendue par un autre nœud.
