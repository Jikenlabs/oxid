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

## 🐳 Option 2 : Déploiement avec Docker Compose (Recommandé)

Pour disposer de la visionneuse avec support complet de tous les formats (y compris RTF, vieux Office et documents complexes) :

### Fichier `docker-compose.yml` minimal :

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

Lancer l'environnement :
```bash
docker compose up -d
```
L'interface de la visionneuse est immédiatement accessible sur [http://localhost:8080](http://localhost:8080) (et la page de démo sur [http://localhost:8080/demo-component.html](http://localhost:8080/demo-component.html)).

---

### Profils alternatifs disponibles dans le dépôt :

Si vous utilisez le fichier `docker-compose.yml` complet fourni à la racine du dépôt :

* **Profil A (Ultra-léger autonome, RAM < 128 Mo)** :
  ```bash
  docker compose --profile office2pdf up -d
  ```
* **Profil B (Fidélité Office 100% via LibreOffice pur)** :
  ```bash
  docker compose --profile gotenberg up -d
  ```
* **Profil C (Hybride Haute Performance)** :
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

Vous pouvez intégrer la visionneuse Oxid dans n'importe quel portail web, intranet ou application (React, Angular, Vue, Svelte, ou HTML pur) en important son bundle ESM (25 Ko gzip) :

```html
<!-- Chargement du Web Component ESM -->
<script type="module" src="http://localhost:8080/oxid-viewer.es.js"></script>
```

### Cas A : Visualisation via URL directe (`src`)

Idéal pour afficher un document statique ou distribué par un CDN :

```html
<oxid-viewer 
  src="https://monsite.com/documents/contrat.pdf"
  watermark="CONFIDENTIEL — USAGE INTERNE"
  rbac-profile="viewer"
  style="width: 100%; height: 100%;">
</oxid-viewer>
```

---

### Cas B : Utilisation sans `src` (Drag & Drop, Import interactif ou `doc-id`)

Lorsque l'attribut `src` est omis, plusieurs modes d'utilisation sont possibles :

#### 1. Mode Drag & Drop interactif (Écran d'accueil autonome)
Déclarez simplement la balise sans source : Oxid affiche automatiquement son écran d'accueil avec une zone de glisser-déposer prête à l'emploi. L'utilisateur peut y déposer n'importe quel fichier (`.pdf`, `.docx`, `.dxf`, `.dcm`, etc.) :

```html
<!-- Le viewer s'initialise avec l'interface de dépôt de fichier -->
<oxid-viewer style="width: 100%; height: 800px;"></oxid-viewer>
```

#### 2. Chargement programmatique via l'API JavaScript (`File`, `Blob`)
Injectez dynamiquement un fichier depuis un formulaire HTML (`<input type="file">`) ou un flux réseau sans jamais exposer d'URL publique :

```html
<input type="file" id="filePicker" accept=".pdf,.docx,.xlsx,.dxf,.dcm">
<oxid-viewer id="myViewer" style="width: 100%; height: 700px;"></oxid-viewer>

<script>
  const picker = document.getElementById('filePicker');
  const viewer = document.getElementById('myViewer');

  picker.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (file) {
      // Chargement direct de l'objet File en mémoire
      viewer.uploadDocument(file);
    }
  });
</script>
```

#### 3. Chargement par identifiant de document interne (`doc-id`)
Si le document a déjà été converti ou téléversé dans Oxid :

```html
<!-- Chargement direct par doc-id sans spécifier d'URL -->
<oxid-viewer doc-id="06f397394498c2cc-582477ec" style="width: 100%; height: 800px;"></oxid-viewer>
```

---

### Cas C : Utilisation avec une GED (Alfresco, SharePoint, Nextcloud, Paperless, CMIS, S3)

Oxid s'intègre naturellement avec les systèmes de Gestion Électronique de Documents (GED) pour offrir une consultation sécurisée sans fuite de données :

#### 1. Via le connecteur distant natif (`connector` + `doc-id`)
Oxid contacte directement la GED en backend à l'aide de ses connecteurs natifs Rust :

```html
<!-- Connexion directe à une GED CMIS (Alfresco, Nuxeo, OpenKM) -->
<oxid-viewer 
  connector="cmis" 
  doc-id="workspace://SpacesStore/4a8c1f92-7f2e-4a6b-9c2d-98d123456789"
  token="Bearer eyJhbGciOi..."
  watermark="CONSULTATION GED INTERNE"
  rbac-profile="reviewer"
  disable-download
  style="width: 100%; height: 100%;">
</oxid-viewer>

<!-- Connexion à un bucket de stockage GED S3 / MinIO / Ceph -->
<oxid-viewer 
  connector="s3" 
  doc-id="ged-bucket/contrats/2026/fournisseur_acme.docx"
  style="width: 100%; height: 100%;">
</oxid-viewer>
```

#### 2. Sauvegarde et synchronisation des annotations XFDF vers la GED
Capturez les annotations, signatures et biffures créées par l'utilisateur pour les enregistrer directement dans les métadonnées de votre GED :

```javascript
const viewer = document.querySelector('oxid-viewer');

// Événement déclenché lors de la sauvegarde d'annotations
viewer.addEventListener('oxid:annotation-saved', async (event) => {
  const xfdfPayload = event.detail; // Flux XML standard XFDF (ISO 19005)
  const docId = viewer.getAttribute('doc-id');

  // Envoi vers l'API de votre GED
  await fetch(`/api/ged/documents/${docId}/annotations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/vnd.adobe.xfdf',
      'Authorization': 'Bearer ...'
    },
    body: xfdfPayload
  });
});
```

*Pour déployer un environnement complet avec Paperless-ngx, Nextcloud et un serveur CMIS, consultez le [Guide d'intégration GED](GED_INTEGRATION.md).*

---

### Tableau exhaustif des attributs `<oxid-viewer>` :
| Attribut | Description | Valeurs possibles |
| :--- | :--- | :--- |
| `src` | URL directe du document (optionnel si `doc-id` ou drag & drop utilisé) | `https://...` ou chemin relatif |
| `doc-id` | Identifiant du document dans le cluster Oxid ou dans la GED | UUID v4 ou chemin GED |
| `connector` | Connecteur distant utilisé conjointement avec `doc-id` | `cmis`, `s3`, `filesystem` |
| `token` | Jeton d'authentification Bearer / OIDC transmis à la GED | `Bearer eyJ...` |
| `watermark` | Filigrane dynamique injecté en temps réel | Texte personnalisé |
| `rbac-profile` | Profil de permissions préconfiguré | `admin`, `editor`, `reviewer`, `viewer` |
| `read-only` | Verrouille le document en lecture seule | Présent ou absent |
| `disable-download` | Masque et désactive le bouton de téléchargement du fichier source | Présent ou absent |
| `disable-print` | Empêche l'impression physique et le raccourci `Ctrl+P` | Présent ou absent |
| `disable-redaction` | Masque les outils d'occultation / biffure RGPD | Présent ou absent |
| `disable-sign` | Masque le menu de signature manuscrite / électronique | Présent ou absent |

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

## 🔐 Configuration des Clés API & Sécurisation (API Keys & Quotas)

Pour sécuriser l'accès aux APIs en production et restreindre les consommations par application ou par client :

### 1. Activer les clés côté serveur
Configurez la variable d'environnement `OXID_API_KEYS` avec vos clés et leurs quotas horaires :

```bash
# Exemple en ligne de commande Docker
docker run -d \
  --name oxid-viewer \
  -p 8080:8080 \
  -e OXID_AUTH_REQUIRED=true \
  -e OXID_API_KEYS="sk_prod_app1:5000,sk_client_ged:10000" \
  ghcr.io/jikenlabs/oxid:latest
```

*Format :* `clé1:quota_par_heure,clé2:quota_par_heure`. Si aucun quota n'est spécifié (ex: `OXID_API_KEYS="secret_token"`), le quota par défaut est de 1 000 requêtes / heure.

### 2. Appeler l'API avec votre clé
Les applications clientes peuvent s'authentifier au choix :
* via l'en-tête `X-API-Key` :
  ```bash
  curl -X POST http://localhost:8080/api/convert \
    -H "X-API-Key: sk_prod_app1" \
    -F "file=@devis.docx" \
    -o devis.pdf
  ```
* ou via l'en-tête standard `Authorization: Bearer` :
  ```bash
  curl -X POST http://localhost:8080/api/convert \
    -H "Authorization: Bearer sk_prod_app1" \
    -F "file=@devis.docx" \
    -o devis.pdf
  ```

Pour une description exhaustive des quotas et de la synchronisation distribuée Redis, consultez la [Spécification de l'API de Conversion](CONVERSION_API.md#--authentification--gestion-des-clés-api).

---

## 🌐 Configuration des Origines CORS (Cross-Origin Resource Sharing)

Par défaut, Oxid autorise toutes les origines (`*`) pour faciliter les tests et l'intégration locale. En environnement de production, vous pouvez restreindre les origines autorisées à vos seuls domaines applicatifs.

### Variables d'environnement :
* `OXID_CORS_ALLOWED_ORIGINS` (ou alias `OXID_CORS_ORIGIN`) : Liste des origines autorisées séparées par des virgules (défaut : `*`).
* `OXID_CORS_ALLOW_CREDENTIALS` : `true` ou `false` pour autoriser l'envoi de cookies et d'en-têtes d'authentification (activé par défaut si des origines spécifiques sont déclarées, désactivé si wildcard `*`).

### Exemple de restriction en production :
```bash
docker run -d \
  --name oxid-viewer \
  -p 8080:8080 \
  -e OXID_CORS_ALLOWED_ORIGINS="https://ged.monentreprise.fr,https://app.monentreprise.fr" \
  -e OXID_CORS_ALLOW_CREDENTIALS=true \
  -v oxid_data:/data \
  ghcr.io/jikenlabs/oxid:latest
```

---

## 📚 En Savoir Plus
* 📖 [Documentation Technique Complète](DOCUMENTATION_TECHNIQUE.md)
* 📐 [Dossier d'Architecture & Sécurité](architecture/DOSSIER_ARCHITECTURE.md)
* 📈 [Benchmarks & Performances Haute Charge](BENCHMARK_PERFORMANCES.md)
* 🏢 [Guide d'Intégration GED (Alfresco, SharePoint, CMIS)](GED_INTEGRATION.md)
