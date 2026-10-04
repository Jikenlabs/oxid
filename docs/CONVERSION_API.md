# ⚡ Spécification & Guide Développeur : API de Conversion (Source ➔ PDF)

> **Moteur de Conversion d'Oxid** : Service haute performance et faible latence de conversion unifiée de documents vers le standard **PDF** (Acrobat / PDF-1.4 à PDF-A), avec authentification par clé API optionnelle, contrôle de quotas et mode éphémère (Zero-Retention RGPD).

---

## 🚀 Vue d'Ensemble

L'API de conversion permet aux développeurs et éditeurs d'intégrer une brique de conversion documentaire universelle au sein de leurs applications (ERP, GED, CRM, SaaS), sans avoir à gérer la complexité des formats complexes (Office, CAD/DAO, DICOM médical, Emails).

### Formats Sources Pris en Charge
- 📄 **Bureautique Office** : `DOCX`, `DOC`, `XLSX`, `XLS`, `PPTX`, `PPT`, `ODT`, `ODS`, `ODP`, `RTF`.
- 📐 **Schémas & Diagrammes** : `VSD`, `VSDX` (Microsoft Visio), `ODG` (OpenDocument Draw).
- 🏗️ **CAO / DAO Technique** : `DXF`, `DWG` (AutoCAD avec extraction et rendu des calques vectoriels).
- 🏥 **Santé & Médical** : `DCM`, `DICOM` (CT Scans, IRM, Radiographies avec fenêtrage Hounsfield).
- ✉️ **Messagerie & Archives** : `EML`, `MSG` (RFC 822 / Outlook avec extraction des pièces jointes).
- 📝 **Texte & Balisage** : `TXT`, `MD` (Markdown avec rendu des diagrammes Mermaid), `CSV`, `JSON`, `XML`, `SVG`.

---

## 🔐 Authentification & Gestion des Clés API

Oxid intègre un gestionnaire natif de clés API avec contrôle de quota horaire (Rate Limiting) par clé, opérant soit en mémoire vive (standalone), soit synchronisé via Redis / Valkey (haute disponibilité / cluster multi-nœuds).

### 1. Configuration Serveur des Clés API

Les clés autorisées et leurs quotas horaires sont configurés via les variables d'environnement suivantes :

| Variable d'environnement | Obligatoire | Valeur par défaut | Description |
| :--- | :--- | :--- | :--- |
| `OXID_API_KEYS` | Non | *(vide)* | Liste des clés API et de leur quota horaire au format `clé:quota,clé:quota`. Si le quota est omis, la valeur par défaut est de 1 000 requêtes / heure. |
| `OXID_AUTH_REQUIRED` | Non | `false` (ou `true` si `OXID_API_KEYS` est défini) | Exige impérativement une clé API valide. Si `false` et sans clé fournie, l'accès anonyme est autorisé avec un quota par défaut. |
| `OXID_REDIS_URL` | Non | *(vide)* | URL de connexion Redis/Valkey (`redis://valkey:6379`) pour synchroniser les quotas d'API keys entre plusieurs instances d'Oxid. |

#### Exemples de configuration serveur :

**Via Docker CLI :**
```bash
docker run -d \
  --name oxid \
  -p 8080:8080 \
  -e OXID_AUTH_REQUIRED=true \
  -e OXID_API_KEYS="token_crm_prod:10000,token_ged_alfresco:50000,token_dev_test:1000" \
  ghcr.io/jikenlabs/oxid:latest
```

**Via Docker Compose (`docker-compose.yml`) :**
```yaml
services:
  oxid:
    image: ghcr.io/jikenlabs/oxid:latest
    environment:
      - OXID_AUTH_REQUIRED=true
      - OXID_API_KEYS=sk_live_app1:5000,sk_live_app2:20000
      - OXID_REDIS_URL=redis://valkey:6379
```

---

### 2. Transmission de la Clé API Côté Client

Pour chaque requête adressée aux endpoints protégés (`/api/convert`, `/v1/convert`, etc.), la clé API peut être transmise via l'un des deux en-têtes HTTP suivants :

```http
X-API-Key: sk_live_app1
```
*ou via le standard OAuth2 / Bearer :*
```http
Authorization: Bearer sk_live_app1
```

---

### 3. En-têtes HTTP de Quotas & Rate Limiting renvoyés

Chaque réponse d'Oxid inclut les métadonnées de consommation en temps réel :

| En-tête | Type | Description |
| :--- | :--- | :--- |
| `X-RateLimit-Limit` | Entier | Quota d'appels autorisés par fenêtre horaire (ex: `5000`) |
| `X-RateLimit-Remaining` | Entier | Nombre d'appels restants dans la fenêtre courante de 1 heure |
| `X-RateLimit-Reset` | Timestamp | Timestamp Unix (secondes) de réinitialisation du compteur horaire |
| `X-Converted-By` | Chaîne | Signature et version du moteur (`Oxid-Converter/0.1.1`) |

> ⚠️ **Dépassement de Quota ou Clé Invalide** :
> - En cas de clé absente ou inconnue (si auth requise) : `HTTP 401 Unauthorized`.
> - En cas de dépassement du quota horaire : `HTTP 429 Too Many Requests` avec message JSON détaillant le temps restant avant réinitialisation.

---

## 📡 Endpoint : Conversion One-Shot

### `POST /api/convert` (ou `/v1/convert`)

Convertit un fichier source et renvoie **immédiatement le flux binaire PDF** dans le corps de la réponse.

#### Paramètres d'URL (Query Parameters)
- `watermark` *(optionnel)* : Texte d'un filigrane dynamique rouge semi-transparent à 45° à apposer sur toutes les pages (ex: `?watermark=SPECIMEN%20INTERNE`).
- `ephemeral` *(optionnel, défaut: `true`)* : Si `true`, le fichier et ses structures de travail sont **détruits physiquement de la mémoire et du disque** dès l'envoi de la réponse (RGPD / Secret Bancaire).

#### En-têtes Optionnels
- `X-Watermark` : Alternative à `?watermark=...` pour injecter un filigrane de sécurité.

---

## 💻 Exemples d'Intégration

### 1. cURL (Ligne de commande)

#### Conversion simple (DOCX vers PDF) :
```bash
curl -X POST http://api.oxidrender.com/api/convert \
  -H "X-API-Key: ox_live_votre_cle_123" \
  -F "file=@rapport_financier.docx" \
  --output rapport_financier.pdf
```

#### Conversion d'un plan DAO (DXF vers PDF) avec filigrane :
```bash
curl -X POST "http://api.oxidrender.com/api/convert?watermark=CONFIDENTIEL%20BTP" \
  -H "Authorization: Bearer ox_live_votre_cle_123" \
  -F "file=@plan_etage.dxf" \
  --output plan_filigrane.pdf
```

---

### 2. Node.js / TypeScript (Fetch standard)

```typescript
import fs from 'node:fs';

async function convertDocument(filePath: string, apiKey: string) {
  const fileBuffer = fs.readFileSync(filePath);
  const formData = new FormData();
  formData.append('file', new Blob([fileBuffer]), 'devis.docx');

  const response = await fetch('http://api.oxidrender.com/api/convert?watermark=COPIE%20CLIENT', {
    method: 'POST',
    headers: {
      'X-API-Key': apiKey,
    },
    body: formData,
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Échec de conversion (${response.status}): ${errorText}`);
  }

  console.log(`Conversions restantes ce mois : ${response.headers.get('x-ratelimit-remaining')}`);

  const pdfArrayBuffer = await response.arrayBuffer();
  fs.writeFileSync('devis_converti.pdf', Buffer.from(pdfArrayBuffer));
  console.log("PDF généré avec succès !");
}
```

---

### 3. Python (Requests)

```python
import requests

API_URL = "http://api.oxidrender.com/api/convert"
API_KEY = "ox_live_votre_cle_123"

def convert_to_pdf(input_file_path: str, output_pdf_path: str, watermark: str = None):
    headers = {"Authorization": f"Bearer {API_KEY}"}
    params = {}
    if watermark:
        params["watermark"] = watermark

    with open(input_file_path, "rb") as f:
        files = {"file": (input_file_path, f)}
        resp = requests.post(API_URL, headers=headers, params=params, files=files)

    if resp.status_code == 200:
        with open(output_pdf_path, "wb") as out:
            out.write(resp.content)
        print(f"Conversion réussie ! Quota restant: {resp.headers.get('X-RateLimit-Remaining')}")
    else:
        print(f"Erreur {resp.status_code}: {resp.text}")

# Exemple d'appel
convert_to_pdf("scan_medical.dcm", "radio_poumon.pdf", watermark="PATIENT TEST")
```

---

## 🛠️ Configuration Serveur (Variables d'Environnement)

Pour activer et administrer le service de conversion dans votre déploiement :

| Variable | Description | Valeur par défaut |
| :--- | :--- | :--- |
| `OXID_API_KEYS` | Liste des clés autorisées et quotas horaires (`cle:quota,cle:quota`) | `""` (Mode libre en dev) |
| `OXID_AUTH_REQUIRED` | Force l'obligation d'une clé API valide pour convertir | `true` si `OXID_API_KEYS` est défini |
| `OXID_REDIS_URL` | URL du cluster Redis/Valkey pour les compteurs distribués | `None` (Fallback mémoire local) |
| `OXID_CONVERT_TTL_SECS` | Délai de purge automatique des fichiers résiduels | `900` (15 minutes) |
