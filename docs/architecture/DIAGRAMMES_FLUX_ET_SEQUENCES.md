# 🔄 Modèles de Flux & Diagrammes de Séquence — Oxid

Ce document détaille les cinématiques d'échanges, les protocoles et les diagrammes de séquence des processus clés de la plateforme **Oxid**.

---

## 1. Flux d'Ingestion & Conversion Documentaire

Ce flux décrit le cycle de vie complet depuis la réception d'un document brut jusqu'à sa mise à disposition pour affichage.

```mermaid
sequenceDiagram
    autonumber
    actor Client as Navigateur Web / GED
    participant Gateway as Axum Router / Gateway
    participant Storage as DocumentStorage
    participant Converter as FormatConverter
    participant Engine as PdfEngine
    participant Cache as CacheManager

    Client->>Gateway: POST /api/documents/upload (multipart/form-data)
    Gateway->>Storage: save_document(filename, bytes)
    Storage-->>Gateway: doc_id, filepath
    
    Gateway->>Converter: ensure_pdf_rendition(filepath)
    alt Est déjà un PDF ou une image
        Converter-->>Gateway: effective_path = original_path
    else Est un format CAO (DXF/DWG)
        Converter->>Converter: CadConverter::convert_cad_to_pdf()
        Converter-->>Gateway: effective_path = rendition.pdf
    else Est un format DICOM (.dcm)
        Converter->>Converter: DicomConverter::convert_dicom_to_pdf()
        Converter-->>Gateway: effective_path = rendition.pdf
    else Est un format Vidéo (MP4/WebM)
        Converter->>Converter: ffmpeg -ss 00:00:01 (Poster JPEG)
        Converter-->>Gateway: effective_path = poster.jpg
    else Format Office (DOCX/XLSX)
        Converter->>Converter: OfficeConverter (Natif ou Headless)
        Converter-->>Gateway: effective_path = rendition.pdf
    end

    Gateway->>Engine: get_metadata(doc_id, filename, effective_path)
    Engine-->>Gateway: DocumentMetadata (pages, dimensions, signets)
    
    Gateway->>Cache: Enregistrement métadonnées en cache L1/L2
    Gateway-->>Client: 200 OK (JSON DocumentMetadata)
```

---

## 2. Flux de Rendu avec Cache Hiérarchique & Look-Ahead Worker

Ce flux illustre la récupération d'une page par le viewer et l'anticipation prédictive des pages suivantes par le Web Worker.

```mermaid
sequenceDiagram
    autonumber
    actor User as Utilisateur
    participant UI as Viewer UI (Thread Principal)
    participant Worker as Web Worker (Off-Thread)
    participant Gateway as Axum Gateway
    participant Cache as Cache L1/L2
    participant Renderer as pdftoppm / RenderCore

    User->>UI: Défile vers la Page 5
    UI->>Gateway: GET /api/documents/:id/pages/5/render?dpi=120
    Gateway->>Cache: Recherche clé SHA-256 (Page 5, 120 DPI)
    
    alt Hit Cache L1 (RAM)
        Cache-->>Gateway: Image binaire (< 1 ms)
    else Miss Cache
        Gateway->>Renderer: Rasterisation de la page 5
        Renderer-->>Gateway: Image JPEG turbo (35 ms)
        Gateway->>Cache: Persistance L1 (RAM) + L2 (Disque/Redis)
    end
    
    Gateway-->>UI: Flux Image JPEG (Rendu GPU instantané)
    
    par Anticipation Look-Ahead
        UI->>Worker: postMessage({ type: 'PREFETCH', pages: [6, 7, 8] })
        Worker->>Gateway: GET /api/documents/:id/pages/6/render
        Worker->>Gateway: GET /api/documents/:id/pages/7/render
        Gateway-->>Worker: Données binaires décodées en tâche de fond
        Worker-->>UI: Pages 6 et 7 prêtes en mémoire locale
    end
```

---

## 3. Flux de Filtrage Dynamique des Calques CAO / DAO

Ce flux illustre le recalcul vectoriel à la volée lorsqu'un architecte ou ingénieur masque/affiche des calques métiers.

```mermaid
sequenceDiagram
    autonumber
    actor User as Architecte / Ingénieur
    participant UI as Panneau Calques CAO (Viewer)
    participant Gateway as Axum Gateway
    participant Cad as CadConverter

    User->>UI: Décoche le calque "ELECTRICITE" et "PLOMBERIE"
    UI->>UI: activeCadLayers = ["0", "MURS_PORTEURS", "CLOISONS"]
    UI->>Gateway: GET /api/documents/:id/pages/1/render?dpi=150&layers=0,MURS_PORTEURS,CLOISONS
    
    Gateway->>Gateway: Calcul de la clé de cache spécifique avec paramètre 'layers'
    alt Déjà présent en cache
        Gateway-->>UI: Rendu filtré instantané (< 2 ms)
    else Recalcul nécessaire
        Gateway->>Cad: convert_cad_to_pdf(path, active_layers)
        Cad->>Cad: Filtrage des entités vectorielles en mémoire
        Cad-->>Gateway: Nouveau rendu PDF vectoriel temporaire
        Gateway->>Gateway: Rasterisation pdftoppm
        Gateway-->>UI: Image du plan avec murs isolés (40 ms)
    end
    UI->>UI: Mise à jour sans scintillement de l'image
```

---

## 4. Flux de Streaming Vidéo HTTP Range (RFC 7233)

Ce flux décrit le mécanisme de saut temporel (*seeking / scrubbing*) instantané dans un fichier vidéo sans téléchargement complet.

```mermaid
sequenceDiagram
    autonumber
    actor User as Utilisateur
    participant Player as Lecteur Vidéo HTML5
    participant Gateway as Axum ServeFile
    participant Storage as Disque NVMe

    User->>Player: Déplace le curseur de la timeline à 03:45
    Player->>Gateway: GET /api/documents/:id/video (Header: Range: bytes=15728640-20971519)
    Gateway->>Storage: Seek à l'octet 15 728 640 et lecture de 5 Mo
    Storage-->>Gateway: Données binaires du flux H.264
    Gateway-->>Player: HTTP/1.1 206 Partial Content<br/>Content-Range: bytes 15728640-20971519/78643200<br/>Content-Type: video/mp4
    Player->>Player: Reprise immédiate de la lecture à 03:45 sans attente
```

---

## 5. Flux de Collaboration Temps Réel sur Annotations XFDF

Ce flux illustre la synchronisation bidirectionnelle multi-utilisateurs conforme à la norme ISO 19444-1.

```mermaid
sequenceDiagram
    autonumber
    actor UserA as Utilisateur A (Relecteur)
    actor UserB as Utilisateur B (Signataire)
    participant UI_A as Viewer A
    participant UI_B as Viewer B
    participant Hub as Hub Collaboration (Tokio Broadcast)
    participant Engine as AnnotationEngine
    participant GED as GED / ECM (Alfresco)

    UserA->>UI_A: Trace un surlignage jaune sur la page 2
    UI_A->>UI_A: Génération de l'élément XFDF <highlight>
    UI_A->>Hub: POST /api/documents/:id/annotations (Payload XFDF unitaire)
    
    Hub->>Engine: Enregistrement et fusion dans le XFDF maître
    
    par Diffusion Temps Réel
        Hub-->>UI_B: SSE / WebSocket: { event: 'ANNOT_CREATED', xfdf: '<highlight...>' }
        UI_B->>UI_B: Rendu instantané du surlignage sur l'écran de B
    end

    Hub->>GED: Synchronisation asynchrone CMIS vers la GED
    GED-->>Hub: 200 OK (Version documentaire mise à jour)
```

---

## 6. Flux de Biffure Permanente (Burn-in Redaction RGPD)

Ce flux détaille la destruction physique irréversible des données sensibles sous-jacentes.

```mermaid
sequenceDiagram
    autonumber
    actor DPO as Délégué Protection Données (DPO)
    participant UI as Viewer UI
    participant Redact as RedactionEngine
    participant Lopdf as Moteur Bas-Niveau lopdf
    participant Storage as Stockage Sécurisé

    DPO->>UI: Trace un rectangle de biffure sur un numéro de Sécurité Sociale
    DPO->>UI: Sélectionne le motif "RGPD - Article 17 (Droit à l'effacement)"
    DPO->>UI: Clique sur "Appliquer les biffures définitivement"
    
    UI->>Redact: POST /api/documents/:id/redact (Coordonnées x, y, w, h, motif)
    Redact->>Lopdf: Chargement du dictionnaire PDF en mémoire
    
    Note over Redact,Lopdf: 1. Détection des glyphes et flux textuels (Tj/TJ)<br/>2. Destruction physique des octets de caractères<br/>3. Suppression des flux d'images sous la zone<br/>4. Dessin d'un polygone noir opaque avec texte du motif
    
    Lopdf-->>Redact: Document PDF expurgé de tout octet sensible
    Redact->>Storage: Enregistrement du nouveau fichier PDF conforme
    Redact-->>UI: 200 OK (Statut: Redaction appliquée irréversiblement)
    UI->>UI: Rechargement du document certifié sans fuite
```
