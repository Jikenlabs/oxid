# 💻 Guide du Développeur & d'Intégration — Oxid

Ce guide fournit toutes les instructions nécessaires pour **développer, étendre et intégrer Oxid** dans vos applications.

---

## 1. Environnement de Développement

### A. Prérequis Système
- **Rust** : Version 1.80+ (avec `cargo`).
- **Node.js** : Version 18+ ou 20+ (avec `npm`).
- **Utilitaires Système recommandés** :
  - `pdftoppm` (issu du paquet `poppler-utils` sous Linux / macOS) : Rendu haute fidélité.
  - `ffmpeg` & `ffprobe` : Extraction de miniatures et métadonnées vidéo.
  - `libreoffice` (optionnel) : Pour la conversion bureautique headless des formats complexes hérités.

Sous Ubuntu / Debian :
```bash
sudo apt update
sudo apt install -y build-essential curl poppler-utils ffmpeg libreoffice
```

---

## 2. Structure du Code Source

```
oxid/
├── backend/                  # Serveur API REST & Moteurs de conversion (Rust)
│   ├── Cargo.toml            # Dépendances Rust (Axum, Tokio, lopdf, image)
│   ├── src/
│   │   ├── main.rs           # Point d'entrée, configuration et démarrage du serveur HTTP
│   │   ├── api/routes.rs     # Définition des endpoints REST & WebSockets
│   │   ├── cache.rs          # Gestionnaire de cache L1 (RAM) et L2 (Disque/Redis)
│   │   ├── connectors/       # Connecteurs de stockage (CMIS, S3, Filesystem, HTTP)
│   │   ├── engine/           # Moteurs métier
│   │   │   ├── pdf.rs        # Moteur PDF, extraction texte et rasterisation
│   │   │   ├── annotations.rs# Sérialiseur / Désérialiseur XFDF standard ISO
│   │   │   ├── redaction.rs  # Moteur de biffure physique (burn-in vectoriel)
│   │   │   ├── forms.rs      # Moteur de formulaires interactifs AcroForms
│   │   │   ├── converter/    # Convertisseurs spécifiques
│   │   │   │   ├── cad.rs    # Moteur vectoriel CAO/DAO (DXF/DWG) & Calques
│   │   │   │   ├── dicom.rs  # Moteur médical DICOM & Fenêtrage Hounsfield
│   │   │   │   ├── office.rs # Moteur bureautique Office (Docx/Xlsx/Pptx)
│   │   │   │   ├── email.rs  # Moteur d'emails EML/MSG avec pièces jointes
│   │   │   │   └── text.rs   # Moteur texte, CSV, logs et Markdown + Mermaid
├── frontend/                 # Client Web Component & UI Viewer (TypeScript + Vite)
│   ├── package.json          # Scripts de build Vite & TypeScript
│   ├── src/
│   │   ├── viewer.ts         # Contrôleur principal du viewer et gestion d'état
│   │   ├── oxid-element.ts   # Définition du Web Component standard <oxid-viewer>
│   │   ├── template.ts       # Gabarits HTML et icônes SVG de l'interface
│   │   ├── style.css         # Thèmes sombre/clair, styles de calques et player vidéo
│   │   ├── render-worker.ts  # Web Worker pour décodage et préchargement asynchrone
├── docs/                     # Documentation technique, fonctionnelle et guides
└── deploy/                   # Fichiers de déploiement Docker, Compose et Helm Kubernetes
```

---

## 3. Compilation & Exécution Locale

### Lancer le Backend en Mode Développement
```bash
cd backend
cargo run
# Le serveur démarre sur http://localhost:8080
```

### Compiler le Backend en Mode Release (Production)
```bash
cd backend
cargo build --release
# Le binaire optimisé est disponible sous : ./target/release/oxidrender
```

### Lancer le Frontend en Mode Développement (Hot-Reload)
```bash
cd frontend
npm install
npm run dev
```

### Compiler le Frontend (Bundle UI + Bibliothèque Web Component)
```bash
cd frontend
npm run build:all
# Met à jour automatiquement les assets statiques distribués dans ../backend/static/
```

---

## 4. Guide d'Intégration Frontend

### Option 1 : Utilisation du Web Component `<oxid-viewer>` (Recommandé)

Le Web Component est 100% autonome et compatible avec n'importe quelle stack (**React, Angular, Vue, Svelte, HTML pur**).

#### Intégration en HTML Pur / Vanilla JS
```html
<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>Mon Application</title>
  <!-- 1. Feuilles de styles et bundle JS du viewer -->
  <link rel="stylesheet" href="http://localhost:8080/assets/index.css">
  <script type="module" src="http://localhost:8080/assets/index.js"></script>
</head>
<body style="margin: 0; height: 100vh; overflow: hidden;">

  <!-- 2. Déclaration de la balise <oxid-viewer> -->
  <oxid-viewer
    id="viewer"
    server-url="http://localhost:8080"
    document-id="VOTRE_DOCUMENT_ID"
    theme="dark"
    scroll-mode="page"
    style="width: 100%; height: 100%; display: block;">
  </oxid-viewer>

  <script>
    const viewer = document.getElementById('viewer');

    // Écouter les événements du lecteur
    viewer.addEventListener('documentloaded', (e) => {
      console.log('Document chargé avec succès :', e.detail);
    });

    viewer.addEventListener('pagechanged', (e) => {
      console.log('Page actuelle :', e.detail.page);
    });
  </script>
</body>
</html>
```

---

#### Intégration dans React (JSX / TSX)
```tsx
import React, { useEffect, useRef } from 'react';

// Si importé comme package npm :
// import '@oxidrender/viewer';
// import '@oxidrender/viewer/style.css';

export const DocumentViewer: React.FC<{ docId: string }> = ({ docId }) => {
  const viewerRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const el = viewerRef.current;
    if (!el) return;

    const onPageChange = (e: any) => console.log('Nouvelle page :', e.detail.page);
    el.addEventListener('pagechanged', onPageChange);

    return () => el.removeEventListener('pagechanged', onPageChange);
  }, []);

  return (
    <div style={{ width: '100%', height: '100vh' }}>
      <oxid-viewer
        ref={viewerRef}
        server-url="http://localhost:8080"
        document-id={docId}
        theme="dark"
        style={{ width: '100%', height: '100%', display: 'block' }}
      />
    </div>
  );
};
```

---

#### Intégration dans Angular
Dans votre `app.module.ts` ou composant Standalone, autorisez les Web Components :
```typescript
import { NgModule, CUSTOM_ELEMENTS_SCHEMA } from '@angular/core';

@NgModule({
  schemas: [CUSTOM_ELEMENTS_SCHEMA]
})
export class AppModule {}
```
Puis dans le template HTML :
```html
<oxid-viewer
  [attr.server-url]="serverUrl"
  [attr.document-id]="docId"
  (documentloaded)="onDocumentLoaded($event)"
  style="width: 100%; height: 100%; display: block;">
</oxid-viewer>
```

---

### Attributs HTML du Web Component

| Attribut | Type | Défaut | Description |
| :--- | :--- | :--- | :--- |
| `server-url` | `string` | `""` *(même hôte)* | URL racine du serveur Oxid. |
| `document-id` | `string` | `""` | Identifiant unique du document à afficher. |
| `theme` | `dark` \| `light` | `dark` | Thème graphique de l'interface. |
| `scroll-mode` | `page` \| `continuous`| `page` | Mode d'affichage page par page ou défilement continu. |
| `watermark` | `string` | `""` | Texte du filigrane de sécurité dynamique appliqué à l'affichage. |

---

### Méthodes Publiques de l'API JavaScript

Vous pouvez contrôler le lecteur par programmation :

```typescript
const viewer = document.getElementById('myViewer');

// Navigation
viewer.goToPage(3);
viewer.nextPage();
viewer.prevPage();

// Zoom & Rotation
viewer.setZoom(1.5); // Zoom 150%
viewer.fitWidth();   // Ajuster à la largeur
viewer.fitPage();    // Ajuster à la page
viewer.rotate(90);   // Rotation de 90 degrés

// CAO / DAO (Calques techniques)
const layers = viewer.getCadLayers();
viewer.toggleCadLayer('MURS_PORTEURS', true); // Activer un calque
viewer.setCadLayers(['0', 'CLOISONS']);        // Rendu exclusif

// Médical DICOM
viewer.setDicomWindow(40, 80); // Cerveau (WC=40, WW=80)
viewer.toggleDicomCine();      // Démarrer / arrêter la boucle ciné 15 fps

// Formulaires
viewer.saveFormValues();       // Sauvegarder les saisies

// Annotations
viewer.downloadXfdf();         // Télécharger les annotations XFDF
```

---

### Événements DOM Émis

| Événement | `event.detail` | Déclenché quand... |
| :--- | :--- | :--- |
| `documentloaded` | `{ id, filename, page_count, pages }` | Le document est prêt et rendu. |
| `pagechanged` | `{ page, totalPages }` | L'utilisateur navigue vers une autre page. |
| `zoomchanged` | `{ zoom }` | Le niveau d'agrandissement est modifié. |
| `annotcreated` | `{ annotation }` | Une nouvelle annotation est dessinée. |
| `cadlayerschange`| `{ layers }` | La liste des calques visibles est modifiée. |
| `formsaved` | `{ updated_fields_count }` | Les données du formulaire ont été persistées. |

---

## 5. Guide d'Extension Backend (Ajout d'un Format)

Pour ajouter la prise en charge d'un nouveau format dans le moteur Rust :

1. **Déclarer le format dans `backend/src/engine/converter/mod.rs`** :
   ```rust
   match ext.as_str() {
       "monformat" => {
           mon_convertisseur::convert_to_pdf(path, &rendition_path)?;
       }
       // ...
   }
   ```
2. **Implémenter le convertisseur dans `backend/src/engine/converter/`** :
   Le convertisseur prend le fichier d'entrée et génère soit un flux PDF intermédiaire, soit une image raster.
3. **Recompiler et tester** :
   ```bash
   cargo check --manifest-path backend/Cargo.toml
   ```

---

## 6. Déploiement en Production

### Variables d'Environnement

| Variable | Défaut | Description |
| :--- | :--- | :--- |
| `OXID_PORT` | `8080` | Port d'écoute HTTP du serveur. |
| `OXID_HOST` | `0.0.0.0` | Adresse réseau d'écoute. |
| `OXID_DATA_DIR` | `./data` | Répertoire de stockage local des documents et du cache L2. |
| `OXID_CORS_ALLOWED_ORIGINS` | `*` | Origines autorisées pour CORS, séparées par virgule (ex: `https://app.domaine.fr,http://localhost:3000`). |
| `OXID_CORS_ALLOW_CREDENTIALS` | `false` si `*`, sinon `true` | Autorise l'envoi de cookies et d'en-têtes d'authentification (`Access-Control-Allow-Credentials`). |
| `OXID_OFFICE_ENGINE` | `hybrid` | Moteur bureautique (`office2pdf`, `gotenberg` ou `hybrid`). |
| `OXID_GOTENBERG_URL` | *(vide)* | URL de l'instance Gotenberg en cas de mode `gotenberg` ou `hybrid`. |
| `OXID_REDIS_URL` | *(vide)* | URL de connexion au cache distribué Redis/Valkey (`redis://redis:6379`). |
| `OXID_CMIS_URL` | *(vide)* | URL du référentiel CMIS (ex: Alfresco). |
| `OXID_API_KEYS` | *(vide)* | Liste des clés API et quotas horaires (`cle1:quota,cle2:quota`). |
| `OXID_AUTH_REQUIRED` | `false` | Impose la présence d'une clé API valide pour accéder à l'API. |
| `RUST_LOG` | `oxid=info,tower_http=info` | Niveau de verbosité des logs. |

### Lancer avec Docker Compose
```bash
docker-compose up -d --build
```
L'application démarre immédiatement avec une consommation mémoire inférieure à 50 Mo.
