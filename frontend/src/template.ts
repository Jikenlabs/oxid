export const VIEWER_TEMPLATE = `
  <div class="oxid-viewer-root">
    <!-- Barre d'outils supérieure unifiée en ligne unique -->
    <header class="toolbar toolbar-unified">
      <!-- Groupe gauche : Logo, bascule panneau latéral, titre, navigation, zoom, rotation, disposition -->
      <div class="tool-group">
        <span class="logo-badge">Oxid</span>
        <button class="btn btn-icon-only" id="btnToggleSidebar" title="Afficher/masquer panneau latéral (Ctrl+B)">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><line x1="9" y1="3" x2="9" y2="21"/></svg>
        </button>

        <div class="divider"></div>

        <span class="doc-title" id="docTitle" title="Nom du document">Aucun document chargé</span>

        <div class="divider"></div>

        <!-- Navigation entre les pages -->
        <button class="btn btn-icon-only" id="btnPrevPage" title="Page précédente (Flèche Gauche ou Haut)">◀</button>
        <input type="number" class="page-input" id="pageNumberInput" value="1" min="1">
        <span style="font-size: 12px; color: var(--text-muted); white-space: nowrap;">/ <span id="pageCountLabel">1</span></span>
        <button class="btn btn-icon-only" id="btnNextPage" title="Page suivante (Flèche Droite ou Bas)">▶</button>

        <div class="divider"></div>

        <!-- Contrôles de zoom -->
        <button class="btn btn-icon-only" id="btnZoomOut" title="Zoom arrière">−</button>
        <span id="zoomLevelLabel" style="font-size: 12px; min-width: 38px; text-align: center;">100%</span>
        <button class="btn btn-icon-only" id="btnZoomIn" title="Zoom avant">+</button>
        <button class="btn" id="btnFitWidth" title="Ajuster à la largeur">Largeur</button>
        <button class="btn" id="btnFitPage" title="Ajuster à la page">Page</button>

        <div class="divider"></div>

        <!-- Groupe de rotation avec sous-menu -->
        <div class="rotate-group" id="rotateGroup" style="position: relative; display: flex; align-items: center; gap: 2px;">
          <button class="btn btn-icon-only" id="btnRotateCcw" title="Rotation 90° gauche (Tout le document)">↺</button>
          <button class="btn btn-icon-only" id="btnRotateCw" title="Rotation 90° droite (Tout le document)">↻</button>
          <button class="btn btn-icon-only" id="btnRotateMenu" type="button" title="Options de rotation (Tout le document / Page active)" style="padding: 0 4px;">
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" style="pointer-events: none;"><path d="m6 9 6 6 6-6"/></svg>
          </button>

          <!-- Sous-menu de rotation -->
          <div class="toolbar-dropdown-menu" id="rotateMenu" style="display: none;">
            <div class="dropdown-header">Portée par défaut</div>
            <button class="dropdown-item active" id="menuOptRotateAll" type="button">
              <span class="dropdown-check" id="checkRotateAll">✓</span>
              <span class="dropdown-label">Tout le document</span>
              <span class="dropdown-badge">Défaut</span>
            </button>
            <button class="dropdown-item" id="menuOptRotateCurrent" type="button">
              <span class="dropdown-check" id="checkRotateCurrent">&nbsp;</span>
              <span class="dropdown-label" id="rotateCurrentPageLabel">Page active (P. 1)</span>
            </button>

            <div class="dropdown-divider"></div>
            <div class="dropdown-header">Action rapide page active</div>
            <button class="dropdown-item action-item" id="menuActionRotatePageCw" type="button">
              <span class="dropdown-icon">↻</span>
              <span class="dropdown-label" id="actionRotateCwLabel">Tourner page 1 (90° droite)</span>
            </button>
            <button class="dropdown-item action-item" id="menuActionRotatePageCcw" type="button">
              <span class="dropdown-icon">↺</span>
              <span class="dropdown-label" id="actionRotateCcwLabel">Tourner page 1 (90° gauche)</span>
            </button>
          </div>
        </div>

        <div class="divider"></div>

        <!-- Disposition & Plein écran -->
        <button class="btn btn-icon-only active" id="btnLayoutSingle" title="Mode page unique">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="6" y="3" width="12" height="18" rx="2"/><line x1="9" y1="7" x2="15" y2="7"/><line x1="9" y1="11" x2="15" y2="11"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnLayoutDouble" title="Mode double page (Livre)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="4" width="9" height="16" rx="1"/><rect x="13" y="4" width="9" height="16" rx="1"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnLayoutGrid" title="Mode grille (Planche contact)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
        </button>

        <div class="divider"></div>

        <!-- Modes de défilement : Continu vs Page par page -->
        <button class="btn btn-icon-only active" id="btnScrollContinuous" title="Défilement continu (glisser de page en page)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v20M17 7l-5-5-5 5M17 17l-5 5-5-5"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnScrollPage" title="Page par page (afficher uniquement la page/planche en cours)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="3" width="16" height="18" rx="2"/><circle cx="12" cy="12" r="1.5"/></svg>
        </button>

        <div class="divider"></div>

        <button class="btn btn-icon-only" id="btnToggleFullscreen" title="Plein écran (F)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnToggleZenMode" title="Mode Zen (Masquer barres d'outils, réapparition au survol) (Z)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
        </button>
      </div>

      <!-- Groupe droit : Outils interactifs, Recherche/PII/Sign/Builder/Compare/Forms et Actions document -->
      <div class="tool-group">
        <!-- Outils interactifs -->
        <button class="btn active" id="toolSelect" title="Outil curseur / sélection">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m3 3 7 18 3-7 7-3L3 3z"/></svg>
          <span class="btn-text">Curseur</span>
        </button>
        <button class="btn" id="toolHighlight" title="Surligner le texte">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m9 11-6 6v3h3l6-6"/><path d="m22 2-7 7 3 3 7-7-3-3z"/></svg>
          <span class="btn-text">Surligner</span>
        </button>
        <button class="btn" id="toolNote" title="Ajouter une note / commentaire">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
          <span class="btn-text">Note</span>
        </button>
        <button class="btn" id="toolRedact" title="Biffure / Masquage permanent de données">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="9" y1="21" x2="9" y2="9"/></svg>
          <span class="btn-text">Biffer</span>
        </button>
        <button class="btn btn-danger" id="btnOpenBurnIn" title="Brûler et sécuriser définitivement les zones biffées" style="display: none;">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><line x1="9" y1="12" x2="15" y2="12"/></svg>
          <span class="btn-text">Appliquer</span>
        </button>

        <div class="divider"></div>

        <button class="btn btn-icon-only" id="btnToggleSearch" title="Rechercher dans le document">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        </button>
        <button class="btn" id="btnOpenPiiScan" title="Détecter automatiquement les données sensibles (RGPD, IBAN, CB, NIR)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
          <span class="btn-text">RGPD</span>
        </button>
        <button class="btn" id="btnOpenSignModal" title="Signer ou apposer un tampon officiel certifié">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 19.5c0 .8-.7 1.5-1.5 1.5H5.5c-.8 0-1.5-.7-1.5-1.5V4.5C4 3.7 4.7 3 5.5 3H12l7 7v9.5z"/><path d="M12 3v7h7"/><path d="m8 15 2 2 4-4"/></svg>
          <span class="btn-text">Signer</span>
        </button>
        <button class="btn btn-icon-only" id="btnOpenBuilder" title="Document Builder (Réorganiser / Fusionner / Filigrane)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnOpenCompare" title="Comparer deux versions d'un document">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 3h5v5"/><path d="M8 21H3v-5"/><path d="M21 3l-7.5 7.5"/><path d="M3 21l7.5-7.5"/></svg>
        </button>
        <button class="btn btn-form-save" id="btnSaveForms" title="Enregistrer les modifications du formulaire interactif" style="display: none;">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
          <span class="btn-text">Enregistrer</span>
        </button>

        <!-- Lecteur boucle cinématographique DICOM (visible lorsqu'un fichier DICOM multi-images est actif) -->
        <div id="dicomCineGroup" style="display: none; align-items: center; gap: 4px; margin-right: 4px;">
          <button class="btn btn-icon" id="btnDicomCinePlay" type="button" title="Lecture boucle cinématographique (Ciné-Run) [Espace]">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" id="dicomCinePlayIcon"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          </button>
          <span style="font-size: 11px; color: var(--text-muted); font-family: monospace; user-select: none;" id="dicomCineFps">15 fps</span>
        </div>

        <!-- Contrôles de contraste / fenêtrage DICOM (visible lorsqu'un fichier DICOM est actif) -->
        <div class="dropdown-container" id="dicomControlsGroup" style="position: relative; display: none; align-items: center;">
          <button class="btn btn-icon-with-text" id="btnDicomPresets" type="button" title="Préréglages médicaux de contraste (Niveaux Hounsfield)">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="m4.93 4.93 4.24 4.24"/><path d="m14.83 9.17 4.24-4.24"/><path d="m14.83 14.83 4.24 4.24"/><path d="m9.17 14.83-4.24 4.24"/></svg>
            <span class="btn-text" id="dicomPresetLabel">Tissus mous</span>
            <span class="dropdown-chevron">▾</span>
          </button>
          <div class="toolbar-dropdown-menu" id="menuDicomPresets" style="display: none; min-width: 275px;">
            <div class="dropdown-header">Fenêtrage Médical (HU)</div>
            <button class="dropdown-item active" data-preset="soft_tissue" type="button">
              <span class="dropdown-check">✓</span>
              <span class="dropdown-label">🫀 Tissus mous</span>
              <span class="dropdown-badge">40 / 400</span>
            </button>
            <button class="dropdown-item" data-preset="lung" type="button">
              <span class="dropdown-check">&nbsp;</span>
              <span class="dropdown-label">🫁 Poumons</span>
              <span class="dropdown-badge">-600 / 1500</span>
            </button>
            <button class="dropdown-item" data-preset="bone" type="button">
              <span class="dropdown-check">&nbsp;</span>
              <span class="dropdown-label">🦴 Os / Squelette</span>
              <span class="dropdown-badge">400 / 1800</span>
            </button>
            <button class="dropdown-item" data-preset="brain" type="button">
              <span class="dropdown-check">&nbsp;</span>
              <span class="dropdown-label">🧠 Cerveau / AVC</span>
              <span class="dropdown-badge">40 / 80</span>
            </button>
            <button class="dropdown-item" data-preset="mediastinum" type="button">
              <span class="dropdown-check">&nbsp;</span>
              <span class="dropdown-label">🫁 Médiastin</span>
              <span class="dropdown-badge">50 / 350</span>
            </button>
          </div>
        </div>

        <div class="divider"></div>

        <!-- Actions sur les fichiers et documents -->
        <input type="file" id="fileUploadInput" style="display: none" accept=".pdf,.png,.jpg,.jpeg,.webp,.tiff,.tif,.bmp,.gif,.svg,.docx,.doc,.xlsx,.xls,.pptx,.ppt,.odt,.ods,.odp,.odg,.vsd,.vsdx,.eml,.msg,.txt,.csv,.tsv,.json,.xml,.md,.dxf,.dwg,.dcm,.dicom">
        <button class="btn btn-primary" id="btnUploadDoc" title="Ouvrir un document">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
          <span class="btn-text">Ouvrir</span>
        </button>
        <button class="btn btn-icon-only" id="btnDownloadDoc" title="Télécharger le document">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        </button>
        <button class="btn btn-icon-only" id="btnPrintDoc" title="Imprimer le document (Ctrl+P)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>
        </button>
      </div>
    </header>

    <!-- Conteneur principal -->
    <div class="main-container">
      <!-- Barre d'activité (style VSCode) -->
      <nav class="activity-bar" id="activityBar" aria-label="Volet d'activités">
        <div class="activity-bar-top">
          <button class="activity-item sidebar-tab active" data-tab="thumbnails" title="Vignettes (Ctrl+1)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/></svg>
          </button>
          <button class="activity-item sidebar-tab" data-tab="bookmarks" title="Plan et signets">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m19 21-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
          </button>
          <button class="activity-item sidebar-tab" data-tab="annotations" title="Annotations et commentaires">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M12 7v4"/><path d="M10 9h4"/></svg>
            <span class="activity-badge" id="annotationsBadge" style="display: none;">0</span>
          </button>
          <button class="activity-item sidebar-tab" data-tab="cad-layers" id="cadLayersTab" style="display: none;" title="Calques CAO / DAO (Layers)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>
            <span class="activity-badge" id="cadLayersBadge" style="display: none;">0</span>
          </button>
          <button class="activity-item sidebar-tab" data-tab="dicom" id="dicomTab" style="display: none;" title="Imagerie Médicale DICOM & Fenêtrage (HU)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/></svg>
            <span class="activity-badge" id="dicomBadge" style="display: none;">CT</span>
          </button>
          <button class="activity-item sidebar-tab" data-tab="forms" id="formsTab" style="display: none;" title="Champs de formulaire interactifs">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="9" y1="13" x2="15" y2="13"/><line x1="9" y1="17" x2="13" y2="17"/></svg>
            <span class="activity-badge" id="formsBadge" style="display: none;">0</span>
          </button>
          <button class="activity-item sidebar-tab" data-tab="attachments" id="attachmentsTab" style="display: none;" title="Pièces jointes">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg>
            <span class="activity-badge" id="attachmentsBadge" style="display: none;">0</span>
          </button>
        </div>
        <div class="activity-bar-bottom">
          <button class="activity-item sidebar-tab" data-tab="info" id="infoTab" title="Propriétés et métadonnées du document">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
          </button>
        </div>
      </nav>

      <!-- Panneau latéral repliable -->
      <aside class="sidebar sidebar-panel" id="appSidebar">
        <div class="sidebar-panel-header">
          <span class="sidebar-panel-title" id="sidebarPanelTitle">VIGNETTES</span>
          <button class="sidebar-panel-close" id="btnCloseSidebar" title="Masquer le volet latéral (Ctrl+B)">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="sidebar-content" id="sidebarContent">
          <!-- Contenu alimenté dynamiquement -->
        </div>
      </aside>

      <!-- Zone d'affichage du document (Viewport) -->
      <main class="viewport" id="documentViewport">
        <div class="empty-state" id="emptyState">
          <svg width="56" height="56" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color: var(--text-muted);"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>
          <p>Glissez un document PDF/Image ou cliquez sur "Ouvrir document"</p>
        </div>
        <div id="pagesContainer" style="display: none; width: 100%; display: flex; flex-direction: column; align-items: center; gap: 20px;">
          <!-- Pages chargées ici -->
        </div>

        <!-- Contrôles de défilement magazine en mode livre -->
        <button class="book-nav-btn book-nav-prev" id="btnBookPrevSpread" title="Page précédente (Flèche gauche)">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <button class="book-nav-btn book-nav-next" id="btnBookNextSpread" title="Page suivante (Flèche droite)">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>
        </button>
      </main>

      <!-- Fenêtre de recherche superposée -->
      <div class="search-overlay" id="searchOverlay" style="display: none;">
        <input type="text" class="search-input" id="searchInput" placeholder="Rechercher...">
        <span id="searchCount" style="font-size: 12px; color: var(--text-muted);">0/0</span>
        <button class="btn" id="btnSearchPrev">▲</button>
        <button class="btn" id="btnSearchNext">▼</button>
        <button class="btn" id="btnCloseSearch">✕</button>
      </div>
    </div>

    <!-- Boîte modale de l'assembleur de documents -->
    <div class="modal-backdrop" id="builderModal" style="display: none;">
      <div class="modal-card">
        <div class="modal-header">
          <h3 class="modal-title">Document Builder (Assemblage & Filigranes)</h3>
          <button class="btn" id="btnCloseBuilder">✕</button>
        </div>
        <div class="modal-body">
          <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 12px;">
            Faites pivoter ou supprimez des pages, puis appliquez un filigrane au vol :
          </p>
          <div style="margin-bottom: 16px; display: flex; gap: 12px; align-items: center;">
            <label style="font-size: 13px;">Filigrane :</label>
            <input type="text" id="builderWatermarkText" class="search-input" placeholder="Ex: CONFIDENTIEL" style="flex: 1;">
          </div>
          <div class="builder-grid" id="builderGrid">
            <!-- Cartes des pages -->
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn" id="btnCancelBuilder">Annuler</button>
          <button class="btn btn-primary" id="btnApplyBuilder">Générer le document</button>
        </div>
      </div>
    </div>

    <!-- Boîte modale de comparaison -->
    <div class="modal-backdrop" id="compareModal" style="display: none;">
      <div class="modal-card" style="max-width: 980px; width: 95%;">
        <div class="modal-header">
          <div style="display: flex; align-items: center; gap: 8px;">
            <h3 class="modal-title">Comparateur Visuel Différentiel Multi-pages</h3>
            <span style="font-size: 11px; background: rgba(59, 130, 246, 0.2); color: var(--accent); padding: 2px 8px; border-radius: 12px; font-weight: 500;">Multi-pages</span>
          </div>
          <button class="btn" id="btnCloseCompare">✕</button>
        </div>
        <div class="modal-body">
          <div style="display: flex; gap: 12px; margin-bottom: 12px; align-items: center; flex-wrap: wrap;">
            <input type="file" id="compareFileInput" accept=".pdf,.png,.jpg,.jpeg,.docx,.tiff,.txt">
            <button class="btn btn-primary" id="btnRunCompare">Comparer avec le document actif</button>
            <span id="compareStats" style="font-size: 13px; font-weight: 600;"></span>
          </div>

          <!-- Barre de navigation de comparaison (Page par page + scan complet) -->
          <div id="compareNavToolbar" style="display: none; margin-bottom: 12px; align-items: center; justify-content: space-between; background: var(--bg-primary); padding: 8px 12px; border-radius: 6px; border: 1px solid var(--border); flex-wrap: wrap; gap: 10px;">
            <div style="display: flex; align-items: center; gap: 8px;">
              <button class="btn" id="btnComparePrevPage" title="Page précédente">◀ Précédente</button>
              <span style="font-size: 12px; font-weight: 500;">Page <strong id="compareCurrentPageLabel">1</strong> / <strong id="compareTotalPagesLabel">1</strong></span>
              <button class="btn" id="btnCompareNextPage" title="Page suivante">Suivante ▶</button>
            </div>

            <!-- Bascule de mode : Diff visuel vs Diff textuel sémantique -->
            <div style="display: flex; gap: 4px; background: rgba(0,0,0,0.3); padding: 2px; border-radius: 6px; border: 1px solid var(--border);">
              <button class="btn active" id="btnModeVisualDiff" style="font-size: 11px; padding: 4px 8px;">🖼️ Diff Visuel</button>
              <button class="btn" id="btnModeTextDiff" style="font-size: 11px; padding: 4px 8px;">📝 Diff Sémantique (Texte)</button>
            </div>

            <!-- Pastilles d'accès rapide aux pages -->
            <div id="comparePageChips" style="display: flex; gap: 6px; align-items: center; overflow-x: auto; max-width: 450px; padding: 2px 0;">
            </div>

            <div style="display: flex; align-items: center; gap: 8px;">
              <button class="btn" id="btnCompareAllPages" title="Calculer le différentiel de toutes les pages">⚡ Scanner tout</button>
              <button class="btn" id="btnToggleContinuousDiff" title="Afficher toutes les pages en défilement continu">Vue continue</button>
            </div>
          </div>

          <div id="compareResultContainer" style="text-align: center; max-height: 520px; overflow: auto; background: #262626; padding: 14px; border-radius: 6px; border: 2px dashed transparent; transition: border-color 0.2s;">
            <p style="color: #bbb; font-size: 13px;">Sélectionnez ou <strong>glissez-déposez ici</strong> un second document pour calculer le différentiel visuel (Vert = Ajouté, Rouge = Supprimé).</p>
          </div>
        </div>
        <div class="modal-footer" style="display: flex; justify-content: space-between; align-items: center;">
          <div style="display: flex; gap: 16px; font-size: 12px; color: var(--text-muted);">
            <span>🟩 <strong>Vert</strong> : Ajouté dans le nouveau document</span>
            <span>🟥 <strong>Rouge</strong> : Supprimé de l'ancien document</span>
            <span>⬜ <strong>Gris</strong> : Contenu identique inchangé</span>
          </div>
          <button class="btn" id="btnCancelCompare">Fermer</button>
        </div>
      </div>
    </div>

    <!-- Boîte modale de confirmation de biffure/caviardage -->
    <div class="modal-backdrop" id="redactModal" style="display: none;">
      <div class="modal-card" style="max-width: 480px;">
        <div class="modal-header">
          <h3 class="modal-title">Appliquer la Biffure Permanente</h3>
          <button class="btn" id="btnCloseRedact">✕</button>
        </div>
        <div class="modal-body">
          <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 12px;">
            Les zones sélectionnées seront <strong>physiquement supprimées</strong> du PDF pour garantir la conformité réglementaire (RGPD).
          </p>
          <div style="margin-bottom: 14px;">
            <label style="font-size: 12px; font-weight: 600;">Motif légal de biffure :</label>
            <select id="redactReasonSelect" class="search-input" style="width: 100%; margin-top: 6px;">
              <option value="RGPD / Données Personnelles">RGPD / Données Personnelles</option>
              <option value="Secret Médical">Secret Médical</option>
              <option value="Secret des Affaires / Confidentiel">Secret des Affaires / Confidentiel</option>
              <option value="Sécurité">Sécurité</option>
            </select>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn" id="btnCancelRedact">Annuler</button>
          <button class="btn btn-danger" id="btnConfirmRedact">Biffer définitivement</button>
        </div>
      </div>
    </div>

    <!-- Boîte modale de l'assistant de détection PII / RGPD -->
    <div class="modal-backdrop" id="piiModal" style="display: none;">
      <div class="modal-card" style="max-width: 680px;">
        <div class="modal-header">
          <div style="display: flex; align-items: center; gap: 8px;">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color: var(--accent);"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
            <h3 class="modal-title">Assistant de Détection & Masquage RGPD / PII</h3>
          </div>
          <button class="btn" id="btnClosePiiModal">✕</button>
        </div>
        <div class="modal-body">
          <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 12px;">
            Oxid analyse les flux textuels et coordonnées géométriques du document pour détecter les données personnelles et bancaires (IBAN SEPA, Cartes Bancaires, Sécurité Sociale NIR, E-mails).
          </p>
          <div id="piiScanLoading" style="text-align: center; padding: 24px 0; display: none;">
            <div style="font-size: 14px; font-weight: 500;">Analyse heuristique & vérification des clés (Luhn, Modulo 97)...</div>
          </div>
          <div id="piiScanEmpty" style="text-align: center; padding: 20px 0; color: var(--text-muted); display: none;">
            ✅ Aucune donnée sensible ou non-conforme détectée dans ce document.
          </div>
          <div id="piiScanResults" style="max-height: 320px; overflow-y: auto; display: none;">
            <!-- Liste des éléments avec cases à cocher -->
          </div>
        </div>
        <div class="modal-footer" style="justify-content: space-between;">
          <span id="piiSummaryCount" style="font-size: 13px; font-weight: 500; color: var(--text-muted);"></span>
          <div style="display: flex; gap: 8px;">
            <button class="btn" id="btnCancelPiiModal">Fermer</button>
            <button class="btn btn-danger" id="btnApplyPiiRedactions" disabled>Biffer la sélection (1-clic)</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Boîte modale de signature numérique & tampon visuel -->
    <div class="modal-backdrop" id="signatureModal" style="display: none;">
      <div class="modal-card" style="max-width: 620px;">
        <div class="modal-header">
          <div style="display: flex; align-items: center; gap: 8px;">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color: var(--accent);"><path d="M20 19.5c0 .8-.7 1.5-1.5 1.5H5.5c-.8 0-1.5-.7-1.5-1.5V4.5C4 3.7 4.7 3 5.5 3H12l7 7v9.5z"/><path d="M12 3v7h7"/><path d="m8 15 2 2 4-4"/></svg>
            <h3 class="modal-title">Signature Électronique & Tampon Officiel</h3>
          </div>
          <button class="btn" id="btnCloseSignModal">✕</button>
        </div>
        <div class="modal-body">
          <div class="sig-tabs" style="display: flex; gap: 10px; margin-bottom: 16px; border-bottom: 1px solid var(--border); padding-bottom: 8px;">
            <button class="btn active" id="tabSigStamp" style="flex: 1;">🏛️ Tampon Certifié</button>
            <button class="btn" id="tabSigHandwritten" style="flex: 1;">✍️ Signature Manuscrite</button>
          </div>

          <!-- Contenu onglet 1 : Tampon officiel certifié -->
          <div id="sigStampContent">
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px;">
              <div>
                <label style="display: block; font-size: 11px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 4px;">Nom du Signataire *</label>
                <input type="text" id="sigSignerName" class="input-ctrl" style="width: 100%;" placeholder="ex: Jean Dupont" value="Jean Dupont" />
              </div>
              <div>
                <label style="display: block; font-size: 11px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 4px;">Motif de Signature</label>
                <input type="text" id="sigReason" class="input-ctrl" style="width: 100%;" placeholder="ex: Approbation légale" value="Approbation légale" />
              </div>
            </div>
            <div style="margin-bottom: 14px;">
              <label style="display: block; font-size: 11px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 4px;">Lieu d'émission</label>
              <input type="text" id="sigLocation" class="input-ctrl" style="width: 100%;" placeholder="ex: Paris, FR" value="Paris, FR" />
            </div>
          </div>

          <!-- Contenu onglet 2 : Tracé manuscrit -->
          <div id="sigHandwrittenContent" style="display: none; margin-bottom: 14px;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
              <label style="font-size: 11px; text-transform: uppercase; color: var(--text-muted);">Tracez votre signature à la souris ou au stylet :</label>
              <button class="btn" id="btnClearCanvas" style="font-size: 11px; padding: 2px 8px;">Effacer</button>
            </div>
            <canvas id="signatureCanvas" width="560" height="140" style="background: #ffffff; border-radius: 6px; border: 1px solid #475569; width: 100%; height: 140px; cursor: crosshair; touch-action: none;"></canvas>
          </div>

          <!-- Positionnement et page cible -->
          <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; padding-top: 10px; border-top: 1px solid var(--border);">
            <div>
              <label style="display: block; font-size: 11px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 4px;">Emplacement du tampon</label>
              <select id="sigPositionPreset" class="input-ctrl" style="width: 100%; padding: 6px;">
                <option value="bottom-right">Bas Droit (Standard)</option>
                <option value="bottom-left">Bas Gauche</option>
                <option value="center">Centre de la page</option>
              </select>
            </div>
            <div>
              <label style="display: block; font-size: 11px; text-transform: uppercase; color: var(--text-muted); margin-bottom: 4px;">Page cible</label>
              <input type="number" id="sigPageNumber" class="input-ctrl" style="width: 100%;" min="1" value="1" />
            </div>
          </div>
        </div>
        <div class="modal-footer" style="display: flex; justify-content: flex-end; gap: 8px;">
          <button class="btn" id="btnCancelSignModal">Annuler</button>
          <button class="btn btn-primary" id="btnApplySignature">Certifier et Signer le document</button>
        </div>
      </div>
    </div>

    <!-- Calque indicateur de progression du chargement du document -->
    <div class="loading-overlay" id="docLoadingOverlay" style="display: none;">
      <div class="loading-box">
        <div class="spinner-ring"></div>
        <h4 id="loadingOverlayTitle">Chargement du document...</h4>
        <p id="loadingOverlaySub">Conversion et optimisation haute fidélité</p>
      </div>
    </div>

    <!-- Calque global de glisser-déposer (Drag & Drop) -->
    <div class="drag-drop-overlay" id="dragDropOverlay" style="display: none;">
      <div class="drag-drop-box">
        <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        <h3>Déposez votre document ici</h3>
        <p>PDF, Images, Word, Excel, PowerPoint, Emails, Textes...</p>
      </div>
    </div>
  </div>
`;
