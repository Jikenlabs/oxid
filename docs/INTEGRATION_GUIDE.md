# Guide d'Intégration & Personnalisation Oxid

Ce guide détaille l'intégration, la personnalisation et l'exploitation d'**Oxid** dans vos environnements applicatifs (Portails GED Alfresco / SharePoint, SPA React, Angular, Vue ou intégration iframe standard).

---

## 1. Vue d'Ensemble & Caractéristiques d'Architecture

| Caractéristique | Architecture Traditionnelle (JVM / Tomcat) | Oxid (Rust / Tokio) | Bénéfice Oxid |
| :--- | :--- | :--- | :--- |
| **Empreinte mémoire** | JVM lourde (4 à 8 Go RAM) | Binaire compilé natif (~42 Mo RAM) | **95% d'économie d'infrastructure** |
| **Temps de démarrage** | 45 à 90 secondes (Cold start JVM) | < 20 millisecondes | **Prêt instantanément (Serverless/Kube)** |
| **Vitesse de rendu** | ~400 à 800 ms par page | ~45 à 75 ms par page (JPEG turbo) | **Fluidité 10x supérieure** |
| **Intégration Web** | Iframe / Applet / JS API lourde | Web Component natif `<oxid-viewer>` & iframe | **Zéro dépendance, framework-agnostic** |
| **Annotations** | Format propriétaire / variable | Format XFDF standard ISO 32000-1 | **Interopérable Acrobat Reader & outils tiers** |
| **Biffure sécurisée** | Burn-in côté serveur | Burn-in vectoriel lopdf natif sans fuite | **Conformité RGPD / secret des affaires** |

---

## 2. Intégration Frontend

### Option A : Web Component `<oxid-viewer>` (Recommandé)
Oxid est encapsulé dans un Web Component autonome standard (`Custom Elements v1`). Il s'intègre en 2 lignes dans n'importe quel code HTML :

```html
<!-- 1. Feuilles de styles et bundle JS du lecteur -->
<link rel="stylesheet" href="http://votre-serveur:8080/assets/index.css" />
<script type="module" src="http://votre-serveur:8080/assets/index.js"></script>

<!-- 2. Utilisation de la balise Web Component -->
<oxid-viewer 
  id="myViewer"
  server-url="http://votre-serveur:8080"
  document-id="DOC_UUID_ICI"
  theme="light"
  style="width: 100vw; height: 100vh; display: block;">
</oxid-viewer>
```

### Option B : Intégration via `<iframe>` (Pattern Web Standard)
Idéal pour Alfresco Share, SharePoint SPFx ou portails intranet existants :

```html
<iframe 
  id="oxidFrame"
  src="http://votre-serveur:8080/?docId=DOC_UUID_ICI&theme=light&toolbar=full"
  width="100%" 
  height="850px" 
  frameborder="0"
  allow="clipboard-read; clipboard-write">
</iframe>
```

---

## 3. Personnalisation & Customisation du Client

### A. Charte Graphique & Thèmes (Variables CSS)
Toutes les couleurs, bordures et espacements du lecteur sont contrôlés par des variables CSS facilement surchargeables :

```css
:root {
  /* Palette personnalisée entreprise */
  --bg-primary: #ffffff;
  --bg-secondary: #f8fafc;
  --paper-bg: #e2e8f0;          /* Fond de la zone de visualisation */
  --accent: #d32f2f;            /* Couleur primaire (ex: rouge marque) */
  --accent-hover: #b71c1c;
  --text-main: #0f172a;
  --text-muted: #64748b;
  --border: #cbd5e1;
}
```

### B. Configuration de la Toolbar par Rôles
Vous pouvez activer ou désactiver chaque fonctionnalité selon le profil utilisateur (ex : lecture seule, superviseur, validateur) :

```javascript
viewer.setConfig({
  readOnly: false,              // Si true: désactive les outils de modification
  allowDownload: true,          // Affiche/masque l'export PDF
  features: {
    highlight: true,            // Surlignage
    notes: true,                // Post-it / notes
    redaction: user.isDpo,      // Biffure réservée au DPO
    digitalSign: user.canSign,  // Signature électronique réservée aux signataires
    piiScan: true,              // Détection automatique RGPD (IBAN, SSN, emails)
    pageBuilder: false,         // Réorganisation / suppression de pages
    compare: true               // Comparaison visuelle de versions
  }
});
```

### C. Filigrane Dynamique de Sécurité (Watermark on-the-fly)
Sécurisation contre les fuites d'écrans ou impressions non autorisées :

```javascript
viewer.setWatermark({
  text: `CONFIDENTIEL - ${currentUser.email} - ${new Date().toLocaleDateString()}`,
  opacity: 0.20,
  color: '#dc2626'
});
```

---

## 4. API Client JavaScript (Méthodes & Événements)

### Contrôle Programmatique
```javascript
const api = viewer.getAPI();

// Navigation & Affichage
api.goToPage(2);
api.nextPage();
api.setZoom(1.5);             // 150%
api.fitWidth();               // Ajuster à la largeur de page
api.fitPage();                // Ajuster à la hauteur de page

// Recherche intégrée
api.search('Périmètre').then(results => {
  console.log(`${results.length} occurrences trouvées`);
  api.highlightNextMatch();
});

// Annotations XFDF
const xfdf = await api.exportXFDF();
await api.importXFDF(xfdf);
```

### Événements (Hooks d'intégration)
```javascript
viewer.addEventListener('documentLoaded', (e) => {
  console.log('Document chargé :', e.detail.filename, 'Total pages :', e.detail.pageCount);
});

viewer.addEventListener('pageChanged', (e) => {
  console.log('Page affichée :', e.detail.pageNumber);
});

viewer.addEventListener('annotationCreated', (e) => {
  console.log('Annotation ajoutée :', e.detail);
});
```

---

## 5. Référence de l'API REST Backend

| Méthode | Endpoint | Description |
| :--- | :--- | :--- |
| `POST` | `/api/documents` | Téléversement d'un document (multipart/form-data) |
| `GET` | `/api/documents/{id}` | Métadonnées (pages, dimensions, signets, pièces jointes) |
| `GET` | `/api/documents/{id}/pages/{p}/render?dpi=120&format=jpeg` | Rendu haute performance d'une page |
| `GET` | `/api/documents/{id}/pages/{p}/thumbnail` | Miniature optimisée avec ratio exact |
| `GET` | `/api/documents/{id}/pages/{p}/text` | Couche texte vectorielle (coordonnées et mots) |
| `GET/POST`| `/api/documents/{id}/annotations` | Récupération / Sauvegarde des annotations XFDF |
| `GET` | `/api/documents/{id}/collab` | Flux Server-Sent Events (SSE) temps réel multi-utilisateurs |
| `POST` | `/api/documents/{id}/redact` | Biffure définitive sécurisée (burn-in) |
| `POST` | `/api/documents/{id}/sign` | Signature électronique PAdES avec tampon visuel |
| `POST` | `/api/documents/build` | Fusion, découpage, rotation et réorganisation de pages |
| `POST` | `/api/documents/compare` | Comparaison visuelle côte-à-côte de 2 documents |
| `GET` | `/api/connectors/open` | Ouverture directe depuis Alfresco (CMIS), S3 ou FS |
| `GET` | `/api/documents/{id}/pii-scan` | Détection automatique des données sensibles (RGPD) |
