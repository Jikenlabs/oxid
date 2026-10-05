# Oxid 🦀📄

<p align="center">
  <img src="./docs/images/viewer_cad_screenshot.png" alt="Oxid Interface" width="92%">
</p>

<p align="center">
  <strong>Moteur de Visualisation & de Conversion Documentaire Universel Haute Performance.</strong><br>
  Serveur asynchrone ultra-léger en <strong>Rust</strong> (Axum, Tokio) & Composant Web autonome en <strong>TypeScript</strong>.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust_1.80+-orange.svg?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Frontend-TypeScript_%26_WebComponent-blue.svg?logo=typescript" alt="TypeScript">
  <img src="https://img.shields.io/badge/Throughput-5.3k+_pages%2Fs_(1_GbE_Wire_Speed)-success.svg" alt="Throughput">
  <img src="https://img.shields.io/badge/p99_Latency-<18_ms-blue.svg" alt="p99 Latency">
  <img src="https://img.shields.io/badge/RAM_Under_Load-<30_MB-brightgreen.svg" alt="RAM Under Load">
  <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License MIT">
</p>

> 🚀 **Démarrage Rapide en 2 minutes :** Consultez notre [**Guide Quickstart (Docker, Compose & Local)**](./docs/QUICKSTART.md) pour lancer Oxid immédiatement.

---

## 📸 Modes de Lecture & Mises en Page (Layouts)

Oxid propose une flexibilité d'affichage avancée adaptée aux différents contextes de travail (revue technique, lecture confortable, tri et navigation rapide) :

| Mode Simple Page avec Vignettes Virtuelles | Mode Vue Grille / Mosaïque Multipage |
| :---: | :---: |
| ![Vignettes & Simple Page](./docs/images/layout_thumbnails_single.png) | ![Vue Grille Multipage](./docs/images/layout_multipage_grid.png) |
| *Affichage linéaire avec barre latérale des vignettes virtualisée (120 FPS)* | *Vue d'ensemble multipage (Planche contact) pour navigation et tri instantané* |

| Mode Double Page / Livre (Spread Reading) | Visualisation Technique CAO / DAO (Calques) |
| :---: | :---: |
| ![Double Page Livre](./docs/images/layout_double_reading.png) | ![CAD Architecture](./docs/images/viewer_cad_screenshot.png) |
| *Affichage côte à côte optimisé pour rapports, revues et brochures* | *Support vectoriel complet AutoCAD avec isolation et filtrage des calques* |

| Photographie Haute Définition Apple (.HEIC) | Icônes & Formats Système macOS (.ICNS) |
| :---: | :---: |
| ![Photo Apple HEIC](./docs/images/viewer_mac_heic_screenshot.png) | ![Icône macOS ICNS](./docs/images/viewer_mac_icns_screenshot.png) |
| *Rendu instantané des photos Apple HEIF/HEIC avec fidelité des couleurs* | *Décodeur natif ICNS multi-résolution jusqu'à 1024x1024 sans perte* |

---

## 📂 Formats et Extensions Pris en Charge

Oxid intègre un pipeline de conversion et de rendu capable de traiter nativement plus de **35 formats documentaires et multimédias** :

```
📁 Formats Sources Supportés
 ├── 📄 Documents PDF & Vectoriels     : .pdf, .svg
 ├── 📝 Bureautique & Documents Office : .docx, .doc, .xlsx, .xls, .pptx, .ppt, .odt, .ods, .odp, .rtf
 ├── 📐 Schémas & Diagrammes           : .vsd, .vsdx (Visio), .odg (OpenDocument Draw)
 ├── 🏗️ Ingénierie & Plans CAO/DAO     : .dxf, .dwg (AutoCAD avec gestion des calques vectoriels)
 ├── 🏥 Santé & Imagerie Médicale PACS : .dcm, .dicom (CT, IRM, Radio avec fenêtrage Hounsfield)
 ├── 🍏 Écosystème Apple & macOS       : .heic, .heif (iPhone/Live Photos), .icns (Icônes macOS), .dng (ProRAW)
 ├── ✉️ Messages & Courriels           : .eml, .msg (RFC 822 / Outlook avec extraction pièces jointes)
 ├── ✍️ Texte & Balisage               : .txt, .md, .markdown, .csv, .tsv, .log, .json, .xml, .yaml, .yml
 ├── 🖼️ Images Haute Définition        : .png, .jpg, .jpeg, .webp, .tiff, .tif, .bmp, .gif
 └── 🎬 Multimédia & Vidéo             : .mp4, .webm, .ogv, .mov, .avi, .mkv, .mp3, .wav, .flac
```

| Famille | Extensions | Fonctionnalités Clés |
| :--- | :--- | :--- |
| **PDF & Images** | `pdf`, `png`, `jpg`, `jpeg`, `webp`, `tiff`, `svg` | Rendu tuilé, text layer vectoriel pour sélection/recherche, zoom fluide 500% |
| **Écosystème Apple / Mac** | `heic`, `heif`, `icns`, `dng`, `pict` | Décodage HEIF/HEIC (iPhone), extraction multi-résolutions ICNS (1024px Retina), conversion PDF |
| **Bureautique Office** | `docx`, `xlsx`, `pptx`, `odt`, `ods`, `odp`, `rtf` | Moteur hybride ultra-rapide (Rust natif / Typst / Gotenberg), préservation typographique |
| **Schémas & Visio** | `vsd`, `vsdx`, `odg` | Extraction vectorielle des formes, connecteurs et organigrammes |
| **Ingénierie CAO/DAO** | `dxf`, `dwg` | Détection automatique des calques, activation/masquage au vol, cotations |
| **Imagerie Médicale** | `dcm`, `dicom` | Rendu 16-bit, fenêtrage interactif Hounsfield (Poumon, Os, Cerveau), lecture Ciné-Loop |
| **Courriels & Emails** | `eml`, `msg` | Restitution fidèle des en-têtes (De, À, Date, Sujet) et panneau des pièces jointes |
| **Code & Markdown** | `md`, `txt`, `csv`, `json`, `xml`, `yaml` | Coloration syntaxique, rendu direct des diagrammes Mermaid vectoriels |
| **Vidéo & Audio** | `mp4`, `webm`, `ogv`, `mov`, `avi`, `mp3` | Streaming HTTP Range RFC 7233 (HTTP 206), extraction de poster à 1s, Picture-in-Picture |

---

## ⚡ Performances & Métriques Clés

Oxid est conçu pour offrir une réactivité maximale tout en réduisant drastiquement l'empreinte infrastructure. 

### 🌐 Résultats Certifiés en Réseau Réel (Campagne v0.1.1 — Liaison Physique 1 GbE) :

Mesures exécutées à travers un réseau commuté physique entre 2 machines distinctes sous plus de **1,5 million de requêtes réelles** :

- 🌐 **Saturation Physique 1 GbE à 100%** : **111 Mo/s continus** de documents utiles transférés (~888 Mbps utiles). Le câble réseau physique est le premier facteur limitant.
- 🏎️ **Débit Utile Réseau (Mixte Production)** : **5 300+ req / seconde** maintenues en flux continu (100% de succès, 0 socket drop).
- ⏱️ **Latence de Queue ($p99$) Maîtrisée** : **$p50 = 7,1\text{ ms}$**, **$p99 = 17,8\text{ ms}$** sous charge soutenue (ratio $p99/p50 < 2,5\times$, distribution quasi-plate).
- 🍃 **Empreinte Mémoire ($VmRSS$)** : **27,3 Mo de RAM seulement** sous charge continue (> 1 million de requêtes traitées sans aucune fuite ni pause Garbage Collector).
- 🛡️ **Haute Disponibilité & Résilience Cluster** : Bascule automatique en **$< 1\text{ ms}$** et 0 erreur HTTP lors du crash brutal d'un nœud en plein pic de charge.
- 💡 **Sobriété FinOps** : **$< 1\text{ vCPU}$** requis pour saturer un lien 1 GbE. Des pods Kubernetes de **256 Mo de RAM / 1 vCPU** suffisent à absorber le pic documentaire d'une administration ou d'un grand compte.
- 📦 **Taille du Binaire / Conteneur** : **~7 Mo natif / ~55 Mo image Docker** (autonome, sans runtime lourd).

> 📊 *Pour les courbes de distribution, la télémétrie seconde par seconde et la méthodologie complète sans omission coordonnée, consultez le [**Rapport Officiel de Benchmark Réseau (v0.1.1)**](docs/RAPPORT_BENCHMARK_OFFICIEL_V0.1.1.md) et le [**Protocole de Benchmark**](docs/PROTOCOLE_BENCHMARK.md).*

---

## 🚀 Moteur de Conversion Headless (Source ➔ PDF)

Oxid intègre une API de conversion directe, ultra-rapide et sécurisée, utilisable directement en backend sans passer par l'interface web :

```bash
# Conversion directe One-Shot avec clé API
curl -X POST http://localhost:8080/api/convert \
  -H "X-API-Key: votre_cle_api" \
  -F "file=@cahier_des_charges.docx" \
  --output cahier_des_charges.pdf
```

Apposez un filigrane dynamique de sécurité :
```bash
curl -X POST "http://localhost:8080/api/convert?watermark=CONFIDENTIEL%20PRO" \
  -H "Authorization: Bearer votre_cle_api" \
  -F "file=@plan_technique.dxf" \
  --output plan_protege.pdf
```

- **Gestion des Quotas Distribués** : Suivi des consommations horaires via Redis (ou mémoire locale).
- **En-têtes HTTP Standards** : `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`.
- **Mode Éphémère (Zero-Retention)** : Suppression physique immédiate des fichiers temporaires après streaming (Conformité RGPD / Secret Professionnel).

Consultez la [Spécification Complète de l'API de Conversion](docs/CONVERSION_API.md) et la [Configuration des Clés API](docs/QUICKSTART.md#--configuration-des-clés-api--sécurisation-api-keys--quotas).

---

## 🧩 Intégration Web Component (`<oxid-viewer>`)

Le composant autonome `<oxid-viewer>` ne pèse que **17 Ko (gzip)** et s'intègre en 2 lignes dans n'importe quelle application (React, Angular, Vue, Svelte, Vanilla) :

```html
<!-- 1. Importer le bundle ESM -->
<script type="module" src="http://localhost:8080/oxid-viewer.es.js"></script>

<!-- 2. Déclarer le composant -->
<oxid-viewer id="myViewer" doc-id="06f397394498c2cc-582477ec" style="width: 100%; height: 800px;"></oxid-viewer>

<!-- 3. Contrôler via l'API JavaScript -->
<script>
  const viewer = document.getElementById('myViewer');

  // Navigation & Zoom
  viewer.goToPage(1);
  viewer.setZoom(1.25);
  viewer.setViewMode('grid'); // 'single' | 'double' | 'grid'

  // Recherche plein texte et occultation RGPD
  viewer.search("facture");
  viewer.runPiiScan();

  // Export
  viewer.download();
</script>
```

Une page de démonstration interactive complète est disponible sur :  
👉 `http://localhost:8080/demo-component.html`

---

## 📚 Documentation Technique & Architecture

Retrouvez les guides détaillés dans le dossier [`docs/`](docs/) :

- 🏛️ **[Dossier d'Architecture Technique (DAT)](docs/architecture/DOSSIER_ARCHITECTURE.md)** : Modèles C4 complets, pipelines et architecture de cache L1/L2.
- 💾 **[Modèle de Données (MCD / MLD)](docs/architecture/MODELE_DONNEES.md)** : Entités, relations, XFDF et structures Redis.
- 🔄 **[Diagrammes de Flux & Séquences](docs/architecture/DIAGRAMMES_FLUX_ET_SEQUENCES.md)** : Ingestion, streaming vidéo, calques CAO et biffure physique.
- 🛡️ **[Architecture de Sécurité & Conformité](docs/architecture/ARCHITECTURE_SECURITE_ET_CONFORMITE.md)** : Zero-Trust, biffure définitive, masquage PII et RGPD.
- ⚡ **[Rapport Officiel de Benchmark Réseau (v0.1.1)](docs/RAPPORT_BENCHMARK_OFFICIEL_V0.1.1.md)** : Campagne certifiée entre 2 machines, saturation 1 GbE et télémétrie continue.
- 🎯 **[Protocole de Benchmark & Métrologie](docs/PROTOCOLE_BENCHMARK.md)** : Méthodologie standardisée exempte d'omission coordonnée.
- 📜 **[Rapport de Benchmark Initial (v0.1.0)](docs/BENCHMARK_PERFORMANCES.md)** : Mesures initiales sur boucle locale.
- 🚀 **[API de Conversion Autonome](docs/CONVERSION_API.md)** : Guide d'intégration, quotas et SDK cURL/Node/Python.
- 📘 **[Documentation Technique](docs/DOCUMENTATION_TECHNIQUE.md)** : Spécification complète des API REST.
- 📕 **[Documentation Fonctionnelle](docs/DOCUMENTATION_FONCTIONNELLE.md)** : Guide exhaustif des fonctionnalités métier.
- 💻 **[Guide Développeur Web Component](docs/DEVELOPER_GUIDE.md)** : Guide d'intégration et événements DOM.

---

## 🛠️ Démarrage Rapide

### Avec Docker Compose (Recommandé)

Créez un fichier `docker-compose.yml` :
```yaml
version: '3.8'

services:
  oxid:
    image: ghcr.io/jikenlabs/oxid:0.2.0
    container_name: oxid
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - OXID_HOST=0.0.0.0
      - OXID_PORT=8080
      - OXID_DATA_DIR=/data
      - OXID_OFFICE_ENGINE=hybrid
      - OXID_GOTENBERG_URL=http://gotenberg:3000
      - OXID_CORS_ALLOWED_ORIGINS=*
    volumes:
      - oxid_data:/data
    depends_on:
      - gotenberg

  gotenberg:
    image: gotenberg/gotenberg:8
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - oxid_data:/data

volumes:
  oxid_data:
```

Lancez avec :
```bash
docker compose up -d
```
L'application est disponible immédiatement sur `http://localhost:8080` (et la démo sur `http://localhost:8080/demo-component.html`).

### En Local (Développement)
```bash
# 1. Compiler le frontend
cd frontend && npm install && npm run build

# 2. Démarrer le serveur Rust
cd ../backend && cargo run --release
```

---

## 📄 Licence

Oxid est un logiciel libre et open source distribué sous licence **MIT** (très permissive pour un usage commercial et privé). Consultez le fichier [LICENSE](LICENSE) pour plus de précisions.
