# 🎯 Protocole Officiel de Benchmark & Métrologie — Oxid (v0.1.1+)

Ce document définit la méthodologie standardisée, reproductible et exempte de biais pour mesurer les performances, la stabilité et les limites de dimensionnement du moteur **Oxid** en conditions réelles de production.

---

## 1. Rétrospective & Analyse Critique du Benchmark v0.1.0

Le premier rapport de benchmark ([`BENCHMARK_PERFORMANCES.md`](BENCHMARK_PERFORMANCES.md)) présentait des débits impressionnants (> 330k req/s sur l'API, 5.46 Go/s de transfert). Cependant, une analyse technique rigoureuse met en lumière quatre angles morts méthodologiques majeurs :

### 1.1. L'explosion du p99 et le phénomène d'« Omission Coordonnée » (*Coordinated Omission*)
* **Observation** : Alors que le p50 était de 1,12 ms (`/api/health`) et 1,82 ms (rendu en cache), le p99 s'effondrait à **412 ms** et **685 ms**, avec des pics maximaux à plus d'une seconde.
* **Causes techniques** :
  1. **Omission coordonnée de `wrk`** : `wrk` fonctionne en boucle fermée (*closed-loop*). Lorsqu'une requête subit une pause ou un temps de traitement long, la connexion attend la réponse avant d'envoyer la suivante. Les requêtes qui *auraient dû* être émises pendant cette seconde de blocage ne sont jamais créées ni mesurées dans l'échantillon, faussant mathématiquement le p99 vers le bas tout en créant une queue de latence bimodalement distribuée.
  2. **Contention de l'ordonnanceur Tokio** : Sous 400 à 1 000 connexions concurrentes saturant 100% du CPU, la file globale et les files locales de *work-stealing* de Tokio s'allongent. Si des micro-tâches synchrones (allocation mémoire, locking de mutex de cache L1, décompression) monopolisent brièvement un thread worker Tokio, les requêtes en attente subissent un retard immédiat de plusieurs centaines de millisecondes.
  3. **Saturation du backlog TCP du kernel Linux** : Avec des valeurs par défaut de `net.core.somaxconn` et `listen_backlog`, l'établissement des sockets sous charge violente subit des retransmissions SYN.

### 1.2. Le biais du test en boucle locale (*Localhost / Loopback*)
* **Ressources partagées** : L'injecteur de charge (12 threads de `wrk`) et le serveur Oxid (24 workers Tokio) s'exécutaient sur le même hôte Linux et se disputaient les 24 cœurs CPU et les caches L1/L2/L3 du processeur, générant un nombre artificiel de *context switches*.
* **Bande passante irréaliste (5,46 Go/s)** : Cette valeur ne correspond pas à un débit réseau mais à une copie en mémoire vive (RAM-to-RAM via l'interface `lo`). En production, une interface réseau standard 10 GbE plafonne à **~1,18 Go/s physique utile**, et 25 GbE à ~2,95 Go/s.
* **Absence de couche TLS/HTTPS** : Le test a été effectué en HTTP clair. Le coût du handshake TLS 1.3, le chiffrement symétrique AES-GCM/ChaCha20 et la négociation ALPN HTTP/2 induisent une charge CPU de 15 à 30% non mesurée initialement.

### 1.3. La durée de test trop courte (10 secondes)
* Une passe de 10 secondes mesure uniquement la réactivité transitoire (*burst*), et non le comportement en régime permanent (*steady-state*).
* **Conséquences ignorées** :
  - **Comportement de l'allocateur mémoire** : Selon l'allocateur (`glibc ptmalloc`, `jemalloc`, `mimalloc`), la fragmentation mémoire et le retour des pages physiques à l'OS (`MADV_DONTNEED`) n'apparaissent qu'après plusieurs dizaines de minutes sous flux soutenu.
  - **Épuisement des ressources système** : Saturation des descripteurs de fichiers (`ulimit -n`), sockets en état `TIME_WAIT`, saturation des tables de conntrack Linux.
  - **Thermal Throttling CPU** : Les fréquences Turbo Boost (ex: 4.8 GHz) chutent souvent à la fréquence de base (ex: 3.2 GHz) après 30 à 60 secondes de charge continue à 100% sur 24 cœurs.

### 1.4. L'asymétrie de comparaison avec la JVM
* Comparer Oxid à une architecture JVM plafonnant à 5 000 req/s correspond à une pile bloquante historique (ex: Tomcat / Spring MVC traditionnel mal configuré).
* Un serveur réactif moderne sur JVM (Vert.x, Quarkus ou Netty / Undertow) atteint facilement 100 000 à 250 000 req/s pour un endpoint de santé sur 24 cœurs.
* **Le véritable avantage d'Oxid sur la JVM doit être recentré là où il est incontestable** :
  - L'empreinte mémoire RSS au repos (< 45 Mo contre 500 Mo à 2 Go pour la JVM).
  - L'absence absolue de pauses Stop-The-World (GC) sous forte pression mémoire.
  - Le temps de démarrage à froid (*Cold Start*) immédiat (< 10 ms contre plusieurs secondes pour la JVM).
  - La consommation électrique et la densité de pods (FinOps).

---

## 2. Évolutions d'Oxid à intégrer dans le nouveau Benchmark

Depuis le benchmark v0.1.0, la plateforme s'est enrichie de fonctionnalités substantielles qui doivent désormais faire l'objet de mesures dédiées :

```
┌────────────────────────────────────────────────────────────────────────┐
│             PÉRIMÈTRE TECHNIQUE DU NOUVEAU PROTOCOLE                   │
├────────────────────────────────────────────────────────────────────────┤
│ 1. API DE CONVERSION ONESHOT  │ POST /api/convert (Office, CAD, DICOM) │
│ 2. CONTRÔLE D'ACCÈS & QUOTAS  │ Clés API, Rate Limiting distribué      │
│ 3. INTÉGRATION GED & STORAGE  │ S3, MinIO, CMIS, WebDAV, Filesystem    │
│ 4. CACHE MULTI-NIVEAUX        │ Cache L1 (RAM) vs Cache L2 (Redis)     │
│ 5. FORMATS LOURDS COMPLEXES   │ Rendu DWG/DXF, Fenêtrage DICOM HU      │
│ 6. ENDPOINTS COLLABORATIFS    │ XFDF, Biffure (Redact), Signature      │
└────────────────────────────────────────────────────────────────────────┘
```

1. **API de conversion à la volée (`POST /api/convert`)** : Test de pipeline bout-en-bout (upload multipart -> conversion multithreadée -> restitution du PDF).
2. **Authentification & Rate Limiting** : Overhead induit par la vérification des en-têtes `X-API-Key` / `Authorization: Bearer` et l'incrémentation atomique des quotas (en mémoire locale et via Redis).
3. **Connecteurs GED distants** : Latence d'ingestion depuis Paperless-ngx, Nextcloud et serveurs CMIS.
4. **Formats non-PDF** : Temps de calcul et consommation CPU pour le fenêtrage Hounsfield des scanners DICOM et la rasterisation multi-calques CAO/DAO.
5. **Opérations sécurisées d'annotations & biffure** : Application de filigranes dynamiques (`?watermark=...`) et brûlage vectoriel de biffures (`/api/documents/:id/redact`).

---

## 3. Architecture d'Épreuve & Pré-requis Matériels

Le protocole exige une isolation stricte entre le générateur de charge et le système sous test (SUT) :

```
┌─────────────────────────┐     Réseau Dédié 10 GbE / 25 GbE      ┌─────────────────────────┐
│     INJECTEUR CHARGE    │ ────────────────────────────────────► │    SYSTÈME SOUS TEST    │
│  (Load Generator Node)  │                                       │       (SUT: Oxid)       │
│  - k6 / wrk2            │ ◄──────────────────────────────────── │  - Oxid v0.1.1 (Rust)   │
│  - Monitoring client    │      HTTP/1.1 & HTTP/2 over TLS       │  - Cache L2 (Redis)     │
│  - Coordinated Omission │                                       │  - Gotenberg / LibreOff │
└─────────────────────────┘                                       └─────────────────────────┘
```

### 3.1. Topologie Réseau & Machines
* **Machine A (Générateur de charge)** :
  - Machine dédiée distincte, minimum 16 vCPU, 32 Go RAM.
  - Outil de charge : **`k6`** (modèle ouvert d'utilisateurs virtuels avec mesure fine des quantiles) ou **`wrk2`** (débit constant avec option `-R`).
* **Machine B (Nœud Oxid / SUT)** :
  - Instance dédiée de production (ex: Bare-metal 16/24 cœurs ou nœud Kubernetes dédié).
* **Interconnexion** : Réseau physique commuté direct minimum **10 GbE** (latence réseau inter-machines < 0,15 ms, MTU 1500 ou Jumbo Frames 9000 documenté).
* **Protocole de transport** : Tests menés sous **HTTPS (TLS 1.3, cipher AES-GCM)** avec certificat valide ou pré-échangé.

### 3.2. Réglages Système (Tuning OS Kernel Linux)
Avant toute mesure, le nœud SUT et l'injecteur doivent appliquer les réglages de production suivants :

```bash
# Augmentation des limites de fichiers et sockets
sysctl -w fs.file-max=2097152
ulimit -n 1048576

# Tuning TCP/IP pour charges extrêmes
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216
```

---

## 4. Les 5 Scénarios de Test Standardisés

Chaque scénario est exécuté selon 3 étapes successives :
1. **Warmup (2 minutes)** : Montée progressive pour peupler les caches et stabiliser les buffers.
2. **Mesure en régime permanent (15 minutes)** : Collecte statistique continue.
3. **Cooldown (1 minute)** : Retour au calme pour vérifier la libération des ressources.

```
Charge (RPS / Concurrence)
   ▲
   │                 ┌─────────────────────────────┐  (Régime permanent - 15 min)
   │                /                               \
   │               /                                 \
   │              / (Warmup - 2 min)                  \ (Cooldown - 1 min)
   └─────────────┴─────────────────────────────────────┴────────► Temps
```

---

### Scénario 1 : Débit Brute & Efficacité Gateway (Health & Auth Check)
* **Objectif** : Mesurer le plafond d'absorption de la gateway Axum/Tokio, avec et sans validation de clé API.
* **Endpoints cibles** :
  1. `GET /api/health` (sans authentification).
  2. `GET /api/health` avec en-tête `X-API-Key: sk_bench_test` (avec quota Redis).
* **Profil de charge** : Palier par paliers de 50k req/s jusqu'à saturation (`target_rps` de 10k à 300k req/s).
* **Métriques attendues** :
  - Seuil de saturation (RPS maximum maintenu).
  - Dérive du p99 (doit rester < 15 ms avant le point de saturation).
  - Coût CPU unitaire de la validation d'une clé API.

---

### Scénario 2 : Rendu Documentaire en Cache (Hit L1 RAM & Hit L2 Redis)
* **Objectif** : Mesurer les performances de consultation massive de pages déjà générées.
* **Endpoints cibles** :
  - `GET /api/documents/:id/pages/:p/render?dpi=120` (payload moyen : ~60 Ko).
  - `GET /api/documents/:id/pages/:p/thumbnail` (payload moyen : ~8 Ko).
* **Échantillonnage** : Corpus de 100 documents représentatifs (500 pages au total) pour éviter la saturation exclusive d'un seul bloc L1.
* **Profil de charge** : Concurrence de 100 à 1 000 connexions actives soutenues pendant 15 minutes.
* **Métriques attendues** :
  - Débit réseau réel utile (Gbps sortant sur la carte réseau).
  - Latences : p50, p95, p99, p99.9.
  - Taux de hit mémoire vive vs hit disque/Redis.

---

### Scénario 3 : Rendu à Froid & Saturation CPU (Zero-Cache Rasterisation)
* **Objectif** : Évaluer la capacité de calcul pur de la couche de rendu vectoriel (Poppler / `pdftoppm` / décodeur DICOM / CAO).
* **Endpoints cibles** :
  - Pages PDF avec paramètres dynamiques invalidant le cache (`?dpi=131&salt=...`).
  - Scanners DICOM avec fenêtrage Hounsfield dynamique (`?wc=40&ww=80`).
  - Plans CAO avec filtrage dynamique des calques (`?layers=CALQUE_1,CALQUE_2`).
* **Profil de charge** : Modèle ouvert avec taux constant d'injection (de 10 à 250 req/s).
* **Métriques attendues** :
  - Nombre de pages physiques générées par seconde et par vCPU.
  - Temps de calcul moyen par page selon la typologie de document (PDF bureautique, plan d'architecte, scanner CT).
  - Comportement thermique du CPU et stabilité des fréquences.

---

### Scénario 4 : Pipeline d'Ingestion & Conversion Haute Fidélité (`/api/convert`)
* **Objectif** : Mesurer le débit et la consommation mémoire de la conversion de documents bureautiques complexes en PDF.
* **Endpoint cible** : `POST /api/convert` (multipart/form-data).
* **Jeu de données (Corpus de test standardisé)** :
  1. `DOCX_Light` : 2 pages, texte pur (50 Ko).
  2. `DOCX_Heavy` : 45 pages avec tableaux, graphiques et images haute résolution (12 Mo).
  3. `XLSX_MultiSheet` : Feuille de calcul financière de 15 onglets (4 Mo).
  4. `DXF_CAD` : Plan technique AutoCAD avec 30 calques vectoriels (8 Mo).
* **Profil de charge** : Injection contrôlée de 1 à 20 conversions/seconde.
* **Métriques attendues** :
  - Durée moyenne et p95 de conversion par type de fichier.
  - Comportement des processus workers (Gotenberg/LibreOffice) : étanchéité mémoire, recyclage des processus zombies.
  - Débit de traitement quotidien extrapolable (docs/jour).

---

### Scénario 5 : Mixte Réaliste de Production (*Production Blend*)
* **Objectif** : Reproduire l'activité réelle d'une entreprise ou d'une administration sous pic d'activité.
* **Répartition du trafic** :
  - **70%** : Consultation de pages en cache L1/L2 (`/pages/:p/render`).
  - **15%** : Téléchargement de vignettes et navigation latérale (`/pages/:p/thumbnail`).
  - **8%** : Rendu à froid de nouvelles pages non encore en cache.
  - **4%** : Extraction de couche texte et recherche plein texte (`/pages/:p/text`, `/search?q=...`).
  - **2%** : Ingestion et conversion de nouveaux documents (`/api/convert`).
  - **1%** : Opérations d'annotations XFDF et biffure de sécurité (`/annotations`, `/redact`).
* **Durée du test** : **30 minutes d'endurance** (Soak test).
* **Métriques attendues** :
  - Stabilité globale du p99 (absence de dérive temporelle).
  - Évolution du `VmRSS` du processus Oxid et du pool Gotenberg.
  - Détection de toute fuite de mémoire vive ou de descripteurs de fichiers.

---

## 5. Script d'Automatisation de Charge (Exemple k6)

Voici le scénario standard de test à exécuter avec `k6` pour éviter l'omission coordonnée et mesurer fidèlement les quantiles :

```javascript
// bench_oxid_production.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  scenarios: {
    // Test par paliers pour trouver le genou de la courbe
    step_load: {
      executor: 'ramping-arrival-rate',
      startRate: 500,
      timeUnit: '1s',
      preAllocatedVUs: 200,
      maxVUs: 1000,
      stages: [
        { duration: '2m', target: 2000 },  // Préchauffage (Warmup)
        { duration: '10m', target: 5000 }, // Régime permanent nominal
        { duration: '5m', target: 10000 }, // Pic de charge
        { duration: '2m', target: 0 },     // Cooldown
      ],
    },
  },
  thresholds: {
    'http_req_duration': ['p(50)<5', 'p(95)<50', 'p(99)<200'], // SLA cibles stricts
    'http_req_failed': ['rate<0.001'],                          // Moins de 0.1% d'erreurs
  },
};

const BASE_URL = __ENV.TARGET_URL || 'https://oxid-server:8080';
const API_KEY = __ENV.API_KEY || 'sk_bench_test';

export default function () {
  const params = {
    headers: {
      'X-API-Key': API_KEY,
      'Accept': 'image/jpeg',
    },
  };

  // 1. Rendu d'une page
  const res = http.get(`${BASE_URL}/api/documents/bench-doc-1/pages/1/render?dpi=120`, params);
  
  check(res, {
    'status is 200': (r) => r.status === 200,
    'content-type is image': (r) => r.headers['Content-Type'] && r.headers['Content-Type'].includes('image'),
  });

  sleep(0.05);
}
```

Pour lancer le test depuis le nœud injecteur :
```bash
k6 run --out json=results_oxid.json bench_oxid_production.js
```

---

## 6. Grille d'Audit et Critères d'Acceptation SLA

Pour certifier les résultats d'Oxid, chaque rapport de benchmark doit renseigner la grille standard suivante :

| Dimension | Indicateur | Seuil Nominal Requis | Statut d'Acceptabilité |
| :--- | :--- | :--- | :--- |
| **Latence** | Médiane (p50) en cache | $\le 2\text{ ms}$ | Strictement requis |
| **Latence** | 99e percentile (p99) | $\le 50\text{ ms}$ (au lieu de 685 ms) | Strictement requis pour SLA |
| **Latence** | Pic Max (p99.99 / max) | $\le 250\text{ ms}$ | Aucun pic > 500 ms toléré |
| **Disponibilité**| Taux de succès HTTP 2xx | $\ge 99,99\%$ | Zéro tolérance socket drops |
| **Ressources** | Empreinte RAM permanente (RSS)| $\le 256\text{ Mo}$ par pod | Respect du profil FinOps |
| **Ressources** | Stabilité mémoire sur 1h | Variation $\le 10\%$ après warmup | Absence de fuite mémoire |
| **Réseau** | Bande passante physique utile | Documentée en Gbps réels | Mesurée sur NIC physique |
