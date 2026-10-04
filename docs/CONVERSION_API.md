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

## 🔐 Authentification & Quotas

L'accès à l'API de conversion est sécurisé par clé API via l'un des deux en-têtes HTTP suivants :

```http
X-API-Key: votre_cle_api_secrete
```
*ou*
```http
Authorization: Bearer votre_cle_api_secrete
```

### En-têtes HTTP de Quota renvoyés
Chaque réponse d'Oxid inclut les métadonnées de consommation en temps réel (gérées par Redis en cluster distribué ou mémoire locale) :

| En-tête | Type | Description |
| :--- | :--- | :--- |
| `X-RateLimit-Limit` | Entier | Quota d'appels autorisés par fenêtre horaire (ex: `1000`) |
| `X-RateLimit-Remaining` | Entier | Nombre de conversions restantes dans la fenêtre courante |
| `X-RateLimit-Reset` | Timestamp | Timestamp Unix (secondes) de réinitialisation du compteur |
| `X-Converted-By` | Chaîne | Signature du moteur (`Oxid-Converter/0.1.0`) |

En cas de dépassement du quota alloué, l'API répond immédiatement avec le code `HTTP 429 Too Many Requests`.

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
