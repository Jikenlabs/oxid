# Guide d'intégration : Oxid & GEDs Open-Source

Le fichier [`docker-compose.ged.yml`](../docker-compose.ged.yml) permet de faire cohabiter **Oxid** avec **3 GEDs open-source légères** au choix, individuellement ou simultanément.

---

## 1. Vue d'ensemble des 3 Options

| Solution | Type de GED | Port Hôte | RAM | Meilleur cas d'usage |
| :--- | :--- | :--- | :--- | :--- |
| **Paperless-ngx** | GED d'archivage / OCR / indexation | `8000` | ~350 - 500 Mo | Factures, courriers, documents numérisés, classement par tags |
| **Nextcloud** | GED Cloud / WebDAV / Arborescence | `8081` | ~250 - 400 Mo | Arborescences partagées de dossiers, collaboration bureautique |
| **CMIS Server** | ECM Standard ouvert (OASIS CMIS 1.1) | `8082` | ~200 - 300 Mo | Intégration d'entreprise (Alfresco, Nuxeo, OpenKM, SharePoint) |

---

## 2. Démarrage rapide

### Option A : Tout démarrer ensemble (Oxid + les 3 GEDs)
```bash
docker compose -f docker-compose.ged.yml --profile all-ged up -d
```

### Option B : Démarrer une seule GED à la fois
* **Oxid + Paperless-ngx :**
  ```bash
  docker compose -f docker-compose.ged.yml --profile paperless up -d
  ```
* **Oxid + Nextcloud :**
  ```bash
  docker compose -f docker-compose.ged.yml --profile nextcloud up -d
  ```
* **Oxid + GED CMIS :**
  ```bash
  docker compose -f docker-compose.ged.yml --profile cmis up -d
  ```

---

## 3. Accès aux Interfaces Web

| Application | URL locale | Identifiants par défaut |
| :--- | :--- | :--- |
| **Oxid** | [http://localhost:8080](http://localhost:8080) | Accès direct sans authentification |
| **Paperless-ngx** | [http://localhost:8000](http://localhost:8000) | Créer le superuser via CLI (voir ci-dessous) |
| **Nextcloud** | [http://localhost:8081](http://localhost:8081) | Définir l'admin lors du premier chargement web |
| **CMIS Server** | [http://localhost:8082](http://localhost:8082) | Dépôt InMemory accessible sans mot de passe |

### Création du compte admin Paperless-ngx :
```bash
docker exec -it paperless-web python3 manage.py createsuperuser
```

---

## 4. Connexion et Visualisation dans Oxid

### Cas 1 : Ouvrir un document depuis Paperless-ngx
Une fois un document indexé (par exemple avec l'ID `1`), Oxid peut l'ouvrir directement :
```
http://localhost:8080/?url=http://paperless-web:8000/api/documents/1/download/
```

### Cas 2 : Ouvrir un document depuis Nextcloud (Lien public ou WebDAV)
* Depuis un lien de partage public :
  ```
  http://localhost:8080/?url=http://nextcloud-ged/s/TOKEN/download
  ```
* Ou via WebDAV avec authentification :
  ```
  http://localhost:8080/?url=http://user:password@nextcloud-ged/remote.php/webdav/MonDossier/Document.pdf
  ```

### Cas 3 : Ouvrir un document depuis la GED CMIS (Connecteur natif Rust)
Oxid embarque un connecteur natif CMIS 1.1 (`cmis.rs`). Il interroge directement le dépôt CMIS par son identifiant d'objet :
```
http://localhost:8080/api/connectors/open?connector=cmis&id=100
```
*(où `100` est l'ObjectID du document dans le dépôt CMIS).*

---

## 5. Arrêt des conteneurs

Pour arrêter tous les conteneurs et libérer les ressources :
```bash
docker compose -f docker-compose.ged.yml --profile all-ged down
```
*(Les volumes Docker sont conservés afin que vos documents et configurations ne soient pas perdus).*
