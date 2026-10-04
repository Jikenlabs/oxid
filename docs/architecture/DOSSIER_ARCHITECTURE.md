# 🏛️ Dossier d'Architecture Technique et Logicielle (DAT) — Oxid

---

| **Document** | Dossier d'Architecture Technique et Fonctionnelle (DAT) |
| :--- | :--- |
| **Projet** | Oxid (Moteur & Visualiseur Documentaire Universel) |
| **Version** | 1.0.0 |
| **Statut** | Validé / Production-Ready |
| **Auteurs** | Équipe d'Ingénierie & Architecture Système |
| **Cible** | Architectes SI, Développeurs, Ingénieurs DevOps, Sécurité |

---

## Sommaire
1. [Contexte, Objectifs & Enjeux](#1-contexte-objectifs--enjeux)
2. [Modèle de Contexte Système (C4 - Niveau 1)](#2-modèle-de-contexte-système-c4---niveau-1)
3. [Architecture Fonctionnelle & Découpage Métier](#3-architecture-fonctionnelle--découpage-métier)
4. [Architecture Logique & Conteneurs (C4 - Niveau 2)](#4-architecture-logique--conteneurs-c4---niveau-2)
5. [Architecture des Composants Internes (C4 - Niveau 3)](#5-architecture-des-composants-internes-c4---niveau-3)
6. [Pipeline de Rendition & Moteurs Spécialisés](#6-pipeline-de-rendition--moteurs-spécialisés)
7. [Stratégie de Cache Multi-Niveaux (L1/L2)](#7-stratégie-de-cache-multi-niveaux-l1l2)
8. [Matrice des Flux & Protocoles Réseau](#8-matrice-des-flux--protocoles-réseau)
9. [Architecture Haute Disponibilité (HA) & Reprise d'Activité (PRA)](#9-architecture-haute-disponibilité-ha--reprise-dactivité-pra)

---

## 1. Contexte, Objectifs & Enjeux

### 1.1 Contexte Métier
Les systèmes de Gestion Électronique de Documents (GED) et d'Enterprise Content Management (ECM) des grands comptes (banques, assurances, santé, secteur public) traitent quotidiennement des millions de pièces hétérogènes. Historiquement, la visualisation s'appuyait sur des solutions Java monolithiques nécessitant des infrastructures surdimensionnées (4 à 16 Go de RAM par serveur) et générant des coûts d'hébergement prohibitifs.

### 1.2 Objectifs Stratégiques d'Oxid
- **Réduction de 95% de l'empreinte mémoire** : Remplacement de la pile Java/JVM par un binaire compilé natif en **Rust**.
- **Universalité Totale** : Prise en charge native d'un spectre documentaire inédit : Bureautique, PDF, CAO/DAO (AutoCAD), Imagerie Médicale (DICOM) et Vidéo sans changer de client.
- **Sécurité et Conformité Réglementaire** : Biffure permanente (Burn-in physique certifié RGPD), signatures horodatées, standardisation ISO 19444-1 (XFDF).
- **Zéro Latence** : Affichage de la première page en moins de 30 ms et débit supérieur à 300 pages/seconde par nœud.

---

## 2. Modèle de Contexte Système (C4 - Niveau 1)

Ce diagramme décrit l'environnement global dans lequel s'insère Oxid et ses interactions avec les systèmes tiers.

```mermaid
C4Context
    title Contexte Système - Écosystème Oxid

    Person(user, "Utilisateur Métier", "Gestionnaire, médecin, ingénieur, juriste consultant ou annotant des documents.")
    System(oxidrender, "Plateforme Oxid", "Moteur de rendition haute performance, visualiseur Web Component universel, streaming et biffures.")
    
    System_Ext(ged, "Systèmes GED / ECM", "Alfresco, IBM FileNet, OpenText Documentum, SharePoint, Nuxeo via CMIS 1.1.")
    System_Ext(storage, "Stockage Objets / SAN", "AWS S3, MinIO, Ceph, baies de stockage NVMe.")
    System_Ext(redis, "Cache Distribué", "Cluster Redis / Valkey pour partage d'état et cache L2 inter-nœuds.")
    System_Ext(iam, "Système d'Identité & SSO", "Keycloak, Entra ID / Active Directory (OAuth2 / OIDC / SAML).")

    Rel(user, oxidrender, "Consulte, annote, signe, compare, lit des flux vidéo", "HTTPS / WSS")
    Rel(oxidrender, ged, "Synchronise les métadonnées et annotations XFDF", "CMIS 1.1 / REST")
    Rel(oxidrender, storage, "Lecture / Écriture des documents originaux et renditions", "S3 API / POSIX")
    Rel(oxidrender, redis, "Stockage du cache partagé L2 et sessions collaboratives", "RESP Protocol (TCP 6379)")
    Rel(oxidrender, iam, "Validation des jetons d'autorisation", "Bearer JWT / RFC 7519")
```

---

## 3. Architecture Fonctionnelle & Découpage Métier

L'architecture fonctionnelle est découpée en 8 domaines autonomes :

```mermaid
graph TD
    subgraph INGESTION["1. Ingestion & Connecteurs"]
        I1["API Upload Multipart"]
        I2["Connecteur CMIS 1.1"]
        I3["Connecteur S3/MinIO"]
        I4["Connecteur Filesystem"]
        I5["Connecteur URL/HTTP"]
    end

    subgraph RENDITION["2. Pipeline de Rendition"]
        R1["Normaliseur PDF"]
        R2["Moteur Vectoriel CAO/DAO"]
        R3["Moteur Médical DICOM (PACS)"]
        R4["Moteur Multimédia (Range 206)"]
        R5["Moteur Bureautique Hybride"]
        R6["Moteur Emails & Pièces Jointes"]
    end

    subgraph CACHING["3. Stratégie de Cache"]
        C1["Cache L1 (RAM LRU < 1ms)"]
        C2["Cache L2 (Disque/Redis SHA-256)"]
        C3["Invalidation Déterministe"]
    end

    subgraph COLLAB["4. Collaboration & Annotations"]
        A1["Parser/Générateur XFDF (ISO 19444-1)"]
        A2["Hub WebSocket / SSE Temps Réel"]
        A3["Synchronisation GED Déportée"]
    end

    subgraph SECURITY["5. Sécurité & Conformité"]
        S1["Biffure Permanente (Burn-in Vectoriel)"]
        S2["Signature Électronique SHA-256"]
        S3["Filigrane Dynamique Anti-Fuite"]
        S4["Scanner PII (RGPD / IBAN / CB)"]
    end

    subgraph FORMS["6. Formulaires Intelligents"]
        F1["Extracteur AcroForms"]
        F2["Injection Dynamique de Données"]
        F3["Aplatissement (Flattening)"]
    end

    INGESTION --> RENDITION
    RENDITION --> CACHING
    CACHING --> COLLAB
    CACHING --> SECURITY
    CACHING --> FORMS
```

---

## 4. Architecture Logique & Conteneurs (C4 - Niveau 2)

Le diagramme ci-dessous illustre la répartition des conteneurs, leurs dépendances et protocoles de communication.

```mermaid
C4Container
    title Architecture des Conteneurs - Plateforme Oxid

    Container(spa, "Interface Web / Client", "TypeScript, Web Component, Shadow DOM, CSS Scoped", "Fournit l'expérience utilisateur dans le navigateur avec rendu multi-couches.")
    
    Container_Boundary(backend_cluster, "Cluster Applicatif Oxid (Stateless)") {
        Container(api_gateway, "Axum HTTP Gateway", "Rust / Axum 0.7", "Gestion des connexions, streaming vidéo Range 206, compression Gzip, CORS.")
        Container(rendition_core, "Moteur de Rendition", "Rust / Rayon / Tokio", "Traitement vectoriel, rasterisation pdftoppm, décodage DICOM, conversion DXF.")
        Container(collab_hub, "Hub Collaboration", "Rust / Tokio Broadcast", "Synchronisation bidirectionnelle des annotations multi-utilisateurs.")
    }

    ContainerDb(redis_cluster, "Cluster Valkey / Redis", "In-Memory Data Store", "Cache partagé L2 distribué et coordination de session.")
    ContainerDb(storage_volume, "Volume de Données", "NFS v4 / CephFS / AWS EFS", "Stockage persistant des documents d'origine et rendus.")

    Rel(spa, api_gateway, "Requêtes REST, Tuiles d'images, Streaming vidéo", "HTTPS / JSON / Binary")
    Rel(spa, collab_hub, "Événements d'annotation en direct", "WSS / SSE")
    Rel(api_gateway, rendition_core, "Délégation asynchrone non-bloquante", "Tokio Tasks")
    Rel(rendition_core, redis_cluster, "Lecture / Écriture cache L2", "TCP 6379")
    Rel(rendition_core, storage_volume, "I/O Disque POSIX", "File System API")
```

---

## 5. Architecture des Composants Internes (C4 - Niveau 3)

### 5.1 Composants Backend (Rust)
Le backend est organisé selon une architecture hexagonale modulaire :

```
backend/src/
├── main.rs              # Bootstrapper, lecture des variables d'environnement, init Tokio
├── config.rs            # Modèle de configuration typée et validée au démarrage
├── cache.rs             # CacheManager : Abstraction multi-niveaux (L1 mémoire, L2 Redis)
├── api/
│   └── routes.rs        # Définition des endpoints Axum, Handlers HTTP, Streaming
├── connectors/          # Connecteurs GED & Systèmes de fichiers distants
│   ├── traits.rs        # Contrats d'interfaces DocumentConnector, SecurityContext
│   ├── filesystem.rs    # Connecteur local sécurisé avec vérification de path traversal
│   ├── cmis.rs          # Connecteur standard OpenCMIS 1.1 (AtomPub / JSON)
│   └── s3.rs            # Connecteur compatible Amazon S3 & MinIO
├── engine/              # Moteurs de transformation et calculs documentaires
│   ├── pdf.rs           # Extraction de métadonnées, text layer SVG et pdftoppm
│   ├── annotations.rs   # Moteur XFDF ISO 19444-1 (Lecture/Écriture XML)
│   ├── redaction.rs     # Biffure irréversible physique (destruction de primitives lopdf)
│   ├── signature.rs     # Signature numérique vectorielle et cartouche d'audit
│   ├── forms.rs         # Détection, remplissage et aplatissement AcroForms
│   ├── builder.rs       # Assemblage, fusion, rotation, suppression de pages
│   ├── pii.rs           # Analyse lexicale heuristique de données sensibles
│   └── converter/       # Convertisseurs de formats sources
│       ├── cad.rs       # Parser DXF/DWG vectoriel, extraction et filtrage de calques
│       ├── dicom.rs     # Parser DICOM PS3.5, fenêtrage Hounsfield, Ciné loop
│       ├── office.rs    # Conversion bureautique hybride native/headless
│       ├── email.rs     # Parser RFC 822 / MSG avec extraction de pièces jointes
│       └── text.rs      # Formats textuels, CSV et Markdown avec diagrammes Mermaid
└── models/
    └── mod.rs           # Structures de données sérialisables (Serde JSON/XML)
```

### 5.2 Composants Frontend (Web Component)
Le frontend s'appuie sur une architecture multi-couches isolée dans le DOM virtuel :

```mermaid
graph TD
    subgraph VIEWPORT["Viewport de Rendu (DOM Virtuel)"]
        L1["Couche 1 : Raster Graphique GPU (Image JPEG Turbo 120-150 DPI)"]
        L2["Couche 2 : Couche Texte Invisible (Glyphes SVG/HTML sélectionnables)"]
        L3["Couche 3 : Couche Vectorielle Annotations (XFDF Canvas interactif)"]
        L4["Couche 4 : Overlay Formulaire Dynamique (Inputs AcroForms)"]
        L5["Couche 5 : Overlay Filigrane de Sécurité Dynamique"]
    end

    subgraph WORKER["Thread d'Arrière-Plan (Web Worker)"]
        W1["Préchargement Prédictif (Look-ahead +8 / -2 pages)"]
        W2["Décompression Off-Thread des Images"]
        W3["Gestion de la Virtualisation des 1 000+ Vignettes"]
    end

    WORKER -.-> VIEWPORT
```

---

## 6. Pipeline de Rendition & Moteurs Spécialisés

### 6.1 Moteur Imagerie Médicale DICOM (PACS)
- **Pipeline** : Lecture binaire du dataset $\to$ Détection de la Photometric Interpretation (`MONOCHROME1` / `MONOCHROME2`) $\to$ Application des facteurs de pente (*Rescale Slope*) et d'interception (*Rescale Intercept*) $\to$ Application du fenêtrage Hounsfield en mémoire vive $\to$ Projection 8-bit RVB $\to$ Serveur Web.
- **Mode Ciné** : Gestion prédictive des 96 frames angiographiques en boucle fermée à 15 fps avec verrouillage du fond à `#000000` (zéro trame blanche).

### 6.2 Moteur CAO / DAO (Plans AutoCAD)
- **Pipeline** : Décodage du fichier ASCII/Binaire DXF $\to$ Extraction de la table `TABLES/LAYER` $\to$ Parsing de la section `ENTITIES` (Lines, Circles, Arcs, Polylines, Text) $\to$ Détermination de la boîte englobante (*Bounding Box*) $\to$ Rendu sélectif selon le paramètre `?layers=L1,L2`.

### 6.3 Moteur Multimédia Vidéo & Audio
- **Pipeline** : Validation du conteneur (MP4, WebM, MOV) $\to$ Extraction asynchrone du poster frame via `ffmpeg` à $t=1\text{s}$ $\to$ Gestion des en-têtes HTTP `Range: bytes=X-Y` $\to$ Réponse `206 Partial Content` avec `Content-Range`.

---

## 7. Stratégie de Cache Multi-Niveaux (L1/L2)

Le système implémente une stratégie de cache hiérarchique à cohérence stricte :

```mermaid
sequenceDiagram
    autonumber
    actor Client as Navigateur Web
    participant GW as Axum Gateway
    participant L1 as Cache L1 (Mémoire RAM)
    participant L2 as Cache L2 (Redis / Disque NVMe)
    participant Core as Moteur de Rendu Rust

    Client->>GW: GET /api/documents/:id/pages/:p/render?dpi=150&layers=...
    GW->>L1: Vérification clé de cache SHA-256
    alt Hit L1 (Présent en RAM)
        L1-->>Client: Image binaire (< 1 ms)
    else Miss L1
        GW->>L2: Requête cache L2 (Redis / Disque)
        alt Hit L2 (Présent sur disque partagé)
            L2-->>GW: Lecture données persistées (5-10 ms)
            GW->>L1: Enregistrement dans L1 (LRU)
            GW-->>Client: Image binaire
        else Miss L2 (Première demande)
            GW->>Core: Rasterisation / Conversion vectorielle
            Core-->>GW: Génération de l'image (30-50 ms)
            GW->>L2: Persistance asynchrone dans L2
            GW->>L1: Enregistrement dans L1
            GW-->>Client: Image binaire
        end
    end
```

### Format des Clés de Cache
Chaque rendu est indexé sous une clé canonique normalisée :
$$\text{Clé} = \text{SHA256}(\text{DocID} \parallel \text{PageNum} \parallel \text{DPI} \parallel \text{Watermark} \parallel \text{Format} \parallel \text{Layers} \parallel \text{WC} \parallel \text{WW})$$

---

## 8. Matrice des Flux & Protocoles Réseau

| Flux | Source | Destination | Protocole | Port | Description |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Consultation / API** | Client Web | Oxid | HTTPS | `8080` / `443` | Navigation, rendu de tuiles, formulaires, vidéo. |
| **Collaboration Directe** | Client Web | Oxid | WSS / SSE | `8080` / `443` | Propagation temps réel des annotations XFDF. |
| **Cache Partagé L2** | Nœud Oxid | Cluster Redis | TCP / RESP | `6379` | Synchronisation de cache inter-nœuds. |
| **Connecteur GED CMIS** | Nœud Oxid | Alfresco / Nuxeo | HTTPS | `8082` / `443` | Récupération des documents et sauvegarde XFDF. |
| **Stockage Objets S3** | Nœud Oxid | MinIO / AWS S3 | HTTPS | `9000` / `443` | Lecture/écriture des documents archivés. |
| **Supervision / Métriques**| Prometheus | Oxid | HTTP | `8080` (`/api/metrics`)| Collecte des métriques d'observabilité. |

---

## 9. Architecture Haute Disponibilité (HA) & Reprise d'Activité (PRA)

### 9.1 Déploiement en Cluster Sans État (*Stateless*)
Les conteneurs applicatifs Oxid ne conservent aucun état local critique en mémoire :
- Tout l'état de session et de cache est déporté sur le cluster Redis/Valkey.
- Les documents sources résident sur un volume persistant partagé (NFS, CephFS ou S3).
- Un équilibreur de charge (Nginx, Traefik ou Ingress Kubernetes) distribue les flux en Round-Robin avec bascule automatique (*Failover*) sans interruption de service.

### 9.2 Métriques de Dimensionnement & Performances Mesurées

Les métriques ci-dessous sont issues du benchmark officiel certifié (*voir [Rapport de Benchmark](docs/BENCHMARK_PERFORMANCES.md)*) réalisé sur un nœud 24 cœurs :

| Ressource / Métrique | Cluster Traditionnel JVM (2 nœuds) | Cluster Oxid (2 nœuds) | Facteur de Gain Constaté |
| :--- | :---: | :---: | :---: |
| **Allocation Mémoire (RAM)** | 16 Go à 32 Go | **512 Mo à 1 Go** | **Divisé par 30** |
| **Allocation CPU** | 8 à 16 vCPU | **1 à 2 vCPU** | **Divisé par 8** |
| **Taille des Images Docker** | ~4 000 Mo | **~55 Mo** | **Divisé par 70** |
| **Débit API Gateway Maximal** | ~10 000 req/s | **> 330 000 req/s** | **x33 plus rapide** |
| **Débit Rendu Servi (Cache L1/L2)** | ~3 000 p/s | **> 96 000 p/s (5,4 Go/s)** | **x32 plus rapide** |
| **Calcul CPU Brut (Zéro Cache)** | ~30 à 50 p/s | **181 pages physiques / s** | **x4 à x6 plus rapide** |
| **Latence Médiane (p50)** | 150 à 450 ms | **1,12 ms à 1,82 ms** | **Latence divisée par 100 à 250** |
| **Concurrence Soutenue** | ~200 connexions | **1 000+ connexions stables** | **Zéro socket drop, zéro timeout** |
