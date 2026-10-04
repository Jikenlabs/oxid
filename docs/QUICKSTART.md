# 🚀 Guide de Démarrage Rapide (Quickstart) — Oxid

Bienvenue dans le guide de démarrage rapide d'**Oxid**. Ce document vous explique comment lancer, tester et intégrer Oxid en quelques minutes dans votre environnement.

---

## ⚡ Option 1 : Démarrage Instantané via Docker (Recommandé)

Le moyen le plus simple d'exécuter Oxid est d'utiliser le conteneur Docker officiel :

```bash
docker run -d \
  --name oxid-viewer \
  -p 8080:8080 \
  -v oxid_data:/data \
  ghcr.io/jikenlabs/oxid:latest
```

* **Interface Web** : Ouvrez [http://localhost:8080](http://localhost:8080) dans votre navigateur.
* **Vérification de santé** :
  ```bash
  curl http://localhost:8080/api/health
  ```

---

## 🐳 Option 2 : Déploiement avec Docker Compose (Multi-Profils)

Oxid fournit un fichier `docker-compose.yml` préconfiguré avec 3 profils d'exécution selon vos exigences de ressources et de fidélité documentaire :

### Profil A : Ultra-léger autonome (Zéro conteneur externe, RAM < 128 Mo)
Utilise le moteur natif Typst en Rust pour convertir les documents Office sans LibreOffice :
```bash
docker compose --profile office2pdf up -d
```

### Profil B : Fidélité Office 100% (Gotenberg / LibreOffice pur)
Pour une restitution graphique identique au pixel près sur les macros et polices complexes :
```bash
docker compose --profile gotenberg up -d
```

### Profil C : Hybride Haute Performance (Recommandé en Production)
Convertit à la vitesse de l'éclair les documents standards via Rust, et bascule automatiquement sur Gotenberg pour les documents complexes :
```bash
docker compose --profile hybrid up -d
```

---

## 🛠️ Option 3 : Compilation depuis les Sources

### 1. Prérequis Système
* **Rust** (version 1.80+ recommandée) : `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
* **Node.js** (version 18+) & **npm**
* **Outils système de rendu** :
  ```bash
  # Sur Ubuntu / Debian
  sudo apt-get update && sudo apt-get install -y poppler-utils libheif-examples pkg-config libssl-dev
  
  # Sur macOS (Homebrew)
  brew install poppler pkg-config openssl
  ```

### 2. Compilation du Frontend (Web Component & SPA)
```bash
cd frontend
npm install
npm run build:all
cd ..
```

### 3. Lancement du Serveur Backend (Rust)
```bash
cd backend
OXID_HOST=0.0.0.0 OXID_PORT=8080 OXID_FRONTEND_DIR="$(pwd)/static" cargo run --release
```

Le visualiseur est immédiatement accessible sur `http://localhost:8080`.

---

## 🧩 Intégration du Web Component Frontend

Vous pouvez intégrer la visionneuse Oxid dans n'importe quel portail web, intranet ou application (React, Angular, Vue, Svelte, ou HTML pur) :

```html
<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>Mon Application</title>
  <!-- Chargement du Web Component ESM (25 Ko gzip) -->
  <script type="module" src="http://localhost:8080/oxid-viewer.es.js"></script>
</head>
<body style="margin: 0; height: 100vh;">

  <!-- Balise autonome de visualisation -->
  <oxid-viewer 
    src="https://monsite.com/documents/contrat.pdf"
    watermark="CONFIDENTIEL — USAGE INTERNE"
    rbac-profile="viewer"
    style="width: 100%; height: 100%;">
  </oxid-viewer>

  <script>
    const viewer = document.querySelector('oxid-viewer');

    // Écoute des événements d'annotation et de biffure
    viewer.addEventListener('oxid:annotation-saved', (event) => {
      console.log('Annotations sauvegardées (XFDF) :', event.detail);
    });
  </script>
</body>
</html>
```

### Attributs supportés par `<oxid-viewer>` :
| Attribut | Description | Valeurs possibles |
| :--- | :--- | :--- |
| `src` | URL directe du document à visualiser | `https://...` ou chemin relatif |
| `doc-id` | Identifiant d'un document stocké dans le cluster Oxid | UUID v4 |
| `token` | Jeton d'authentification Bearer / OIDC | `Bearer eyJ...` |
| `watermark` | Filigrane dynamique injecté sur toutes les pages | Texte personnalisé |
| `rbac-profile` | Profil de permissions de l'utilisateur | `admin`, `editor`, `reviewer`, `viewer` |

---

## ⚡ Utilisation du Moteur de Conversion CaaS (Headless)

Oxid intègre un moteur de conversion autonome **Source ➔ PDF** utilisable par script ou batch :

```bash
# Convertir un document Word DOCX en PDF avec filigrane et destruction immédiate (Zero-Retention RGPD)
curl -X POST http://localhost:8080/api/convert \
  -F "file=@contrat.docx;filename=contrat.docx" \
  -F "watermark=COPIE DE TRAVAIL" \
  -F "ephemeral=true" \
  -o contrat.pdf
```

Formats convertibles : `.docx`, `.xlsx`, `.pptx`, `.dxf`, `.dwg`, `.dcm` (DICOM), `.eml`, `.msg`, `.heic`, `.icns`, `.svg`, `.txt`, `.md`.

---

## 📚 En Savoir Plus
* 📖 [Documentation Technique Complète](DOCUMENTATION_TECHNIQUE.md)
* 📐 [Dossier d'Architecture & Sécurité](architecture/DOSSIER_ARCHITECTURE.md)
* 📈 [Benchmarks & Performances Haute Charge](BENCHMARK_PERFORMANCES.md)
* 🏢 [Guide d'Intégration GED (Alfresco, SharePoint, CMIS)](GED_INTEGRATION.md)
