# 📕 Documentation Fonctionnelle Complète — Oxid

Ce document présente l'ensemble des fonctionnalités utilisateur et métier offertes par **Oxid**, le viewer et moteur de rendition documentaire universel haute performance.

---

## 1. Présentation & Proposition de Valeur

Oxid est un moteur documentaire universel moderne, léger et réactif. Il s'intègre nativement au sein de vos outils métiers (GED Alfresco, SharePoint, Nuxeo, ERP, CRM ou applications web sur-mesure) pour offrir une expérience de consultation, d'annotation et de conversion documentaire instantanée.

### Les 4 Piliers Fonctionnels
1. **Universalité des Formats** : Un seul viewer pour les PDF, la bureautique Office, les plans d'architecte CAO/DAO, les scanners médicaux DICOM et les vidéos d'expertise.
2. **Conformité & Intégrité** : Standard international ISO pour les annotations (**XFDF ISO 19444-1**) assurant la compatibilité avec Adobe Acrobat Reader et les GED existantes.
3. **Sécurité Forte** : Biffures physiques irréversibles conformes RGPD et Secret Médical, filigranes dynamiques anti-fuite et signatures électroniques horodatées.
4. **Fluidité & Zéro Latence** : Préchargement prédictif et affichage des pages en moins de 50 ms.

---

## 2. Matrice des Formats Supportés

| Catégorie | Extensions | Capacités & Traitements |
| :--- | :--- | :--- |
| **Documents Standards** | `.pdf` | Rendu vectoriel, couche texte sélectionnable, signets, formulaires, pièces jointes. |
| **Bureautique Office** | `.docx`, `.doc`, `.rtf`, `.odt` | Conversion haute fidélité, polices vectorisées, respect des styles et tableaux. |
| **Tableurs** | `.xlsx`, `.xls`, `.csv`, `.tsv`, `.ods` | Pagination automatique, ajustement dynamique des colonnes. |
| **Présentations** | `.pptx`, `.ppt`, `.odp` | Diapositives plein écran, graphiques vectorisés. |
| **Diagrammes** | `.vsd`, `.vsdx`, `.odg` | Schémas Visio et OpenDocument Draw. |
| **Plans CAO / DAO** | `.dxf`, `.dwg` | Plans d'architectes, **gestion des calques techniques** (murs, électricité, plomberie). |
| **Médical & PACS** | `.dcm`, `.dicom` | Scanners CT, IRM, radiographies, **fenêtrage Hounsfield**, **lecteur Ciné angiographie**. |
| **Multimédia Vidéo** | `.mp4`, `.webm`, `.mov`, `.avi`, `.mkv` | Lecteur vidéo intégré HTML5, **streaming HTTP Range**, vitesse variable, PiP, plein écran. |
| **Multimédia Audio** | `.mp3`, `.wav`, `.ogg`, `.flac`, `.aac` | Écoute de dictées médicales, enregistrements d'appels clients. |
| **Messagerie Électronique** | `.eml`, `.msg` | En-têtes (De, À, Date, Objet), corps HTML/texte et **extraction des pièces jointes**. |
| **Documentation & Code** | `.md`, `.json`, `.xml`, `.log`, `.yaml` | Markdown avec rendu des diagrammes **Mermaid.js**, coloration syntaxique. |
| **Images Haute Résolution** | `.png`, `.jpg`, `.jpeg`, `.webp`, `.tiff`, `.bmp` | Rendu optimisé GPU, gestion des TIFF multipages. |

---

## 3. Guide des Fonctionnalités Métier

### A. Navigation & Ergonomie de Consultation
- **Affichage Continu ou Page à Page** : Défilement vertical continu ou navigation focalisée page par page.
- **Gestion du Zoom Dynamique** :
  - Zoom continu de 25% à 500%.
  - Raccourcis automatiques : **Ajuster à la largeur** (`Fit Width`) ou **Ajuster à la page entière** (`Fit Page`).
- **Rotation des Pages** : Rotation horaire / anti-horaire à 90°, 180° et 270°.
- **Volet Latéral d'Exploration** :
  - **Miniatures Virtualisées** : Affichage fluide de documents de plus de 1 000 pages sans saturation mémoire.
  - **Arborescence des Signets (Bookmarks)** : Navigation hiérarchique par chapitres et sections.
  - **Pièces Jointes** : Liste cliquable des pièces jointes d'un email ou d'un PDF avec téléchargement unitaire.
  - **Recherche Plein Texte** : Mise en surbrillance des occurrences trouvées dans tout le document.

---

### B. Annotations & Révision Collaborative (Standard XFDF)
Oxid intègre une palette complète d'outils d'annotation conforme à la norme ISO :
- **Outils Texte** : Surlignage (Highlight), Soulignement (Underline), Biffure simple (Strikeout).
- **Notes Repositionnables (Sticky Notes)** : Ajout de commentaires textuels avec nom de l'auteur et horodatage.
- **Formes Géométriques** : Rectangles, Ellipses, Lignes droites, Flèches directionnelles.
- **Dessin Libre (Ink / Crayon)** : Tracé manuel pour schémas ou annotations manuscrites.
- **Zone de Texte Libre (FreeText)** : Saisie directe de texte sur le document avec choix de couleur.
- **Tampons Métiers (Stamps)** : Tampons prédéfinis (*APPROUVÉ*, *CONFIDENTIEL*, *REFUSÉ*) ou personnalisés.
- **Collaboration Temps Réel** : Toute annotation créée ou modifiée par un utilisateur est propagée en direct aux autres relecteurs connectés sur le même document via WebSocket / SSE.

---

### C. Biffures Légales (Redaction) — Secret Médical & RGPD
Contrairement à un simple rectangle noir graphique (qui laisse le texte sous-jacent copiable dans un PDF), Oxid applique une **biffure physique définitive (Burn-in)** :
1. L'utilisateur trace une zone de biffure et choisit le motif légal (*RGPD Article 17*, *Secret Médical*, *Secret des Affaires*).
2. Lors de l'application, le moteur backend détruit physiquement les pixels et le texte vectoriel sous la zone.
3. Le PDF résultant est garanti sans fuite d'information.

---

### D. Formulaires Interactifs (AcroForms)
- **Détection Automatique des Champs** : Champs texte, zones de saisie multilignes, cases à cocher, boutons radio, listes déroulantes et champs de signature.
- **Saisie Conviviale** : Remplissage directement dans le navigateur avec surbrillance des champs obligatoires.
- **Enregistrement & Aplatissement** :
  - Sauvegarde des valeurs au format standard PDF.
  - Possibilité d'aplatir le formulaire (*Flattening*) pour figer les réponses avant archivage légal.

---

### E. Imagerie Médicale Spécialisée DICOM (PACS)
- **Contraste & Niveaux Hounsfield** : Deux curseurs interactifs (*Window Center* et *Window Width*) pour régler précisément la plage de contraste des scanners scanner CT et radios.
- **Presets Cliniques en 1 Clic** :
  - *Poumons / Thorax* : Visualisation des alvéoles et bronches.
  - *Os / Squelette* : Détection des fractures et densité osseuse.
  - *Cerveau / Tête* : Différenciation matière grise / matière blanche.
  - *Tissus Mous* : Visibilité des organes abdominaux.
- **Lecteur Ciné Angiographie (15 fps)** :
  - Défilement continu en boucle des 96 images d'une coronarographie ou radio d'œil.
  - Raccourci barre d'espace pour démarrer/arrêter l'animation sans latence ni trame blanche.

---

### F. Plans Techniques CAO / DAO (AutoCAD DXF / DWG)
- **Onglet dédié "Calques CAO"** : Liste exhaustive des calques avec pastille de couleur normalisée et nombre d'éléments.
- **Filtrage Interactif** : Masquez par exemple les calques *Électricité* et *Plomberie* pour ne conserver que les *Murs Porteurs*. Le plan se met à jour instantanément.
- **Recherche de calque** et boutons d'action rapide *Tous / Aucun*.

---

### G. Lecteur Vidéo & Multimédia Intégré
- **Lecteur HTML5 Haute Définition** : Lecture fluide des fichiers MP4, WebM, MOV.
- **Streaming sans latence** : Démarre immédiatement sans attendre le téléchargement complet du fichier.
- **Contrôles Professionnels** :
  - Vitesse de lecture ajustable : **0.5x**, **0.75x**, **1.0x**, **1.25x**, **1.5x**, **2.0x**.
  - Raccourcis clavier intuitifs : **Espace** (Lecture/Pause), **F** (Plein écran), **M** (Muet), **Flèches** (Saut ±5s, Volume).
  - Mode **Plein écran immersif** avec masquage automatique du curseur et des contrôles après 2,5 secondes.
  - Mode **Picture-in-Picture** (Incrustation d'image pour continuer à travailler tout en regardant la vidéo).

---

### H. Comparateur de Documents
- **Comparaison Visuelle (Pixel-Diff)** : Affiche en surbrillance rouge les suppressions et en vert les ajouts entre deux versions graphiques d'un même plan ou contrat.
- **Comparaison Textuelle Sémantique** : Analyse mot à mot les modifications textuelles d'un contrat avec rapport des différences.

---

### I. Document Builder (Assemblage & Manipulation)
- **Réorganisation des Pages** : Glisser-déposer pour réordonner les pages.
- **Fusion Multi-Documents** : Combiner un PDF, un document Word et un scan en un seul dossier relié.
- **Extraction & Suppression** : Isoler certaines pages pour créer un nouvel extrait documentaire.
- **Filigrane Dynamique** : Application d'un filigrane de sécurité (*"CONFIDENTIEL"*, nom de l'utilisateur, date).
