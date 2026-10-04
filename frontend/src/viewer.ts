import { AnnotationLayerManager, ClientAnnotation } from './annotation-layer.ts';
import { ComparisonUI } from './comparison-ui.ts';
import { DocumentBuilderUI } from './document-builder-ui.ts';
import { TextLayerRenderer } from './text-layer.ts';
import { FormField, FormFieldsSummary, FormFillResponse, FormLayerRenderer } from './form-layer.ts';


export interface ViewerPermissions {
  canDownload?: boolean;
  canPrint?: boolean;
  canRedact?: boolean;
  canAnnotate?: boolean;
  canSign?: boolean;
  canBuild?: boolean;
  canUpload?: boolean;
  canRotate?: boolean;
  canChangeViewMode?: boolean;
  canChangeScrollMode?: boolean;
  canZenMode?: boolean;
  canSearch?: boolean;
  readOnly?: boolean;
}

export interface DocumentMetadata {
  id: string;
  filename: string;
  mime_type: string;
  file_size: number;
  page_count: number;
  pages: { page_number: number; width: number; height: number; rotation: number }[];
  bookmarks: { title: string; page_number: number }[];
  attachments?: { id: string; filename: string; mime_type: string; size: number }[];
}

export class OxidViewer {
  public permissions: ViewerPermissions = {
    canDownload: true,
    canPrint: true,
    canRedact: true,
    canAnnotate: true,
    canSign: true,
    canBuild: true,
    canUpload: true,
    canRotate: true,
    canChangeViewMode: true,
    canChangeScrollMode: true,
    canZenMode: true,
    canSearch: true,
    readOnly: false,
  };
  public watermarkText: string = '';
  public viewMode: 'single' | 'double' | 'grid' = 'single';
  public scrollMode: 'continuous' | 'page' = 'continuous';
  public zenMode: boolean = false;
  private currentDoc: DocumentMetadata | null = null;
  private currentZoom: number = 1.0;
  private pageRotations: Map<number, number> = new Map();
  private defaultRotation: number = 0;
  private rotateScope: 'all' | 'current' = 'all';
  private activePage: number = 1;
  private currentTool: string = 'select';
  private textRenderers: Map<number, TextLayerRenderer> = new Map();
  private annotationManagers: Map<number, AnnotationLayerManager> = new Map();
  private allAnnotations: ClientAnnotation[] = [];
  private searchMatches: { pageNumber: number; element: HTMLElement }[] = [];
  private currentSearchIndex: number = -1;
  private currentPiiItems: any[] = [];
  private isDrawing: boolean = false;
  private hasDrawnSignature: boolean = false;
  private sigCanvas: HTMLCanvasElement | null = null;
  private sigCtx: CanvasRenderingContext2D | null = null;
  private intersectionObserver: IntersectionObserver | null = null;
  private thumbIntersectionObserver: IntersectionObserver | null = null;
  private renderWorker: Worker | null = null;
  private cachedPageBlobs = new Map<number, string>();
  private cachedThumbBlobs = new Map<number, string>();
  private isUploading: boolean = false;
  private formRenderer: FormLayerRenderer;
  private currentForms: FormFieldsSummary | null = null;
  private activeSidebarTab: string = 'thumbnails';
  private cadMetadata: any | null = null;
  private activeCadLayers: Set<string> = new Set();
  private dicomMetadata: any | null = null;
  private dicomWc: number = 40;
  private dicomWw: number = 400;
  private dicomCustomWindow: boolean = false;
  private dicomWindowingTimeout: any = null;
  private dicomCineInterval: any = null;

  // Root container (either document or specific rootEl)
  private root: HTMLElement | Document = document;

  // DOM elements
  private docTitleEl!: HTMLElement;
  private pageNumberInput!: HTMLInputElement;
  private pageCountLabel!: HTMLElement;
  private zoomLevelLabel!: HTMLElement;
  private pagesContainer!: HTMLElement;
  private emptyState!: HTMLElement;
  private sidebarContent!: HTMLElement;
  private loadingOverlay: HTMLElement | null = null;
  private loadingOverlayTitle: HTMLElement | null = null;
  private loadingOverlaySub: HTMLElement | null = null;

  private escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }

  constructor(rootEl?: HTMLElement) {
    if (rootEl) {
      this.root = rootEl;
    }
    this.formRenderer = new FormLayerRenderer(
      (field, val) => {
        this.dispatchEvent('formfieldchange', { field, value: val });
      },
      (field) => {
        this.activePage = field.page_number;
        this.openSignatureDialog();
      }
    );
    this.initElements();
    this.initRenderWorker();
    this.setupListeners();
    this.checkUrlParams();
  }

  public dispatchEvent(name: string, detail?: any) {
    const target = (this.root && 'dispatchEvent' in this.root) ? this.root : document;
    target.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true, detail }));
  }

  public $(selectorOrId: string): HTMLElement | null {
    const isSelector = selectorOrId.startsWith('#') || selectorOrId.startsWith('.') || selectorOrId.startsWith('[') || selectorOrId.includes(' ');
    const selector = isSelector ? selectorOrId : '#' + selectorOrId;

    if (this.root && 'querySelector' in this.root) {
      return (this.root as HTMLElement).querySelector(selector);
    }
    if (!isSelector) {
      return document.getElementById(selectorOrId);
    }
    return document.querySelector(selector);
  }

  public $$(selector: string): NodeListOf<HTMLElement> {
    if (this.root && 'querySelectorAll' in this.root) {
      return (this.root as HTMLElement).querySelectorAll(selector);
    }
    return document.querySelectorAll(selector);
  }

  private initElements() {
    this.docTitleEl = this.$('docTitle') as HTMLElement;
    this.pageNumberInput = this.$('pageNumberInput') as HTMLInputElement;
    this.pageCountLabel = this.$('pageCountLabel') as HTMLElement;
    this.zoomLevelLabel = this.$('zoomLevelLabel') as HTMLElement;
    this.pagesContainer = this.$('pagesContainer') as HTMLElement;
    this.emptyState = this.$('emptyState') as HTMLElement;
    this.sidebarContent = this.$('sidebarContent') as HTMLElement;
    this.loadingOverlay = this.$('docLoadingOverlay');
    this.loadingOverlayTitle = this.$('loadingOverlayTitle');
    this.loadingOverlaySub = this.$('loadingOverlaySub');
    this.applyPermissionsUI();
  }

  private initRenderWorker() {
    try {
      this.renderWorker = new Worker(new URL('./render-worker.ts', import.meta.url), { type: 'module' });
      this.renderWorker.onmessage = (e: MessageEvent) => {
        const { type, pageNumber, blobUrl } = e.data;
        if (type === 'PAGE_CACHED') {
          this.cachedPageBlobs.set(pageNumber, blobUrl);
          const pageEl = this.$(`page-${pageNumber}`);
          const img = pageEl?.querySelector('.page-image') as HTMLImageElement;
          if (img && !img.src.startsWith('blob:')) {
            img.src = blobUrl;
          }
        } else if (type === 'THUMBNAIL_CACHED') {
          this.cachedThumbBlobs.set(pageNumber, blobUrl);
          const thumbEl = this.$(`thumb-${pageNumber}`);
          const img = thumbEl?.querySelector('img') as HTMLImageElement;
          if (img && !img.src.startsWith('blob:')) {
            img.src = blobUrl;
          }
        }
      };
    } catch (err) {
      console.warn('RenderWorker could not be started', err);
    }
  }

  private prefetchNearbyPages(centerPage: number) {
    if (!this.currentDoc || !this.renderWorker) return;
    const pagesToPrefetch: number[] = [];
    for (let offset = -1; offset <= 4; offset++) {
      const p = centerPage + offset;
      if (p >= 1 && p <= this.currentDoc.page_count && !this.cachedPageBlobs.has(p)) {
        pagesToPrefetch.push(p);
      }
    }
    if (pagesToPrefetch.length > 0) {
      this.renderWorker.postMessage({
        type: 'PREFETCH_PAGES',
        docId: this.currentDoc.id,
        pages: pagesToPrefetch,
        dpi: 120,
      });
    }
  }

  public async loadFromUrl(url: string, token?: string) {
    await this.loadRemote(undefined, undefined, token, url);
  }

  public async loadRemote(connector?: string, docId?: string, token?: string, directUrl?: string) {
    if (this.docTitleEl) this.docTitleEl.textContent = 'Connexion à la GED distante...';
    try {
      const query = new URLSearchParams();
      if (directUrl) query.set('url', directUrl);
      if (connector) query.set('connector', connector);
      if (docId) query.set('id', docId);
      if (token) query.set('token', token);

      const resp = await fetch(`/api/connectors/open?${query.toString()}`);
      if (resp.ok) {
        const meta: DocumentMetadata = await resp.json();
        this.displayDocument(meta);
        const badge = connector ? connector.toUpperCase() : 'URL';
        if (this.docTitleEl) {
          this.docTitleEl.innerHTML = `<span style="background: var(--accent); color: white; padding: 2px 6px; border-radius: 4px; font-size: 11px; margin-right: 6px;">${this.escapeHtml(badge)}</span> ${this.escapeHtml(meta.filename)}`;
        }
      } else {
        const errText = await resp.text();
        if (this.docTitleEl) this.docTitleEl.textContent = 'Échec de chargement distant';
        alert(`Échec de connexion à la GED distante : ${errText || resp.statusText}`);
      }
    } catch (e: any) {
      console.error('Remote open error', e);
      if (this.docTitleEl) this.docTitleEl.textContent = 'Erreur réseau';
      alert(`Erreur réseau lors de l'accès à la GED : ${e?.message || e}`);
    }
  }

  public download() {
    if (!this.currentDoc) return;
    if (this.permissions.canDownload === false || (this.permissions.readOnly && this.permissions.canDownload !== true)) {
      alert('Téléchargement désactivé par la politique de sécurité (RBAC).');
      return;
    }
    const wmParam = this.watermarkText ? `?watermark=${encodeURIComponent(this.watermarkText)}` : '';
    window.open(`/api/documents/${this.currentDoc.id}/download${wmParam}`, '_blank');
  }

  public print() {
    if (!this.currentDoc) return;
    if (this.permissions.canPrint === false || (this.permissions.readOnly && this.permissions.canPrint !== true)) {
      alert('Impression désactivée par la politique de sécurité (RBAC).');
      return;
    }
    window.print();
  }

  public setWatermark(text: string) {
    this.watermarkText = text;
    this.updateAllWatermarks();
  }

  public updateAllWatermarks() {
    const pages = this.pagesContainer?.querySelectorAll('.page-container');
    pages?.forEach((pageCard) => {
      this.attachWatermarkOverlay(pageCard as HTMLElement, this.watermarkText);
    });
  }

  public attachWatermarkOverlay(pageCard: HTMLElement, text: string) {
    const target = (pageCard.querySelector('.page-content') || pageCard) as HTMLElement;
    let wm = target.querySelector('.page-watermark-overlay') as HTMLElement;
    if (!text || text.trim() === '') {
      if (wm) wm.remove();
      return;
    }
    if (!wm) {
      wm = document.createElement('div');
      wm.className = 'page-watermark-overlay';
      wm.style.cssText = 'position: absolute; top: 0; left: 0; width: 100%; height: 100%; pointer-events: none; display: flex; align-items: center; justify-content: center; z-index: 20; user-select: none; overflow: hidden;';
      target.appendChild(wm);
    }
    const escaped = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    wm.innerHTML = `
      <div style="transform: rotate(-35deg); color: rgba(220, 38, 38, 0.18); font-size: clamp(20px, 3.8vw, 42px); font-weight: 800; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; letter-spacing: 2px; text-transform: uppercase; text-align: center; line-height: 1.4; max-width: 88%; word-break: break-word; border: 3px dashed rgba(220, 38, 38, 0.22); padding: 10px 20px; border-radius: 8px;">
        ${escaped}
      </div>
    `;
  }

  public setPermissions(perms: Partial<ViewerPermissions>) {
    this.permissions = { ...this.permissions, ...perms };
    this.applyPermissionsUI();
    this.dispatchEvent('permissionschanged', { permissions: this.permissions });
  }

  public applyPermissionsUI() {
    const isReadOnly = !!this.permissions.readOnly;
    const canDownload = !isReadOnly && this.permissions.canDownload !== false;
    const canPrint = !isReadOnly && this.permissions.canPrint !== false;
    const canRedact = !isReadOnly && this.permissions.canRedact !== false;
    const canAnnotate = !isReadOnly && this.permissions.canAnnotate !== false;
    const canSign = !isReadOnly && this.permissions.canSign !== false;
    const canBuild = !isReadOnly && this.permissions.canBuild !== false;
    const canUpload = !isReadOnly && this.permissions.canUpload !== false;

    // Download & Print
    const btnDownload = this.$('btnDownloadDoc');
    if (btnDownload) btnDownload.style.display = canDownload ? 'inline-flex' : 'none';

    const btnPrint = this.$('btnPrintDoc');
    if (btnPrint) btnPrint.style.display = canPrint ? 'inline-flex' : 'none';

    // Upload
    const btnUpload = this.$('btnUploadDoc');
    if (btnUpload) btnUpload.style.display = canUpload ? 'inline-flex' : 'none';

    // Annotations
    const toolHighlight = this.$('toolHighlight');
    if (toolHighlight) toolHighlight.style.display = canAnnotate ? 'inline-flex' : 'none';

    const toolNote = this.$('toolNote');
    if (toolNote) toolNote.style.display = canAnnotate ? 'inline-flex' : 'none';

    // Redactions
    const toolRedact = this.$('toolRedact');
    if (toolRedact) toolRedact.style.display = canRedact ? 'inline-flex' : 'none';

    const btnBurnIn = this.$('btnOpenBurnIn');
    if (btnBurnIn && !canRedact) btnBurnIn.style.display = 'none';

    const btnPii = this.$('btnOpenPiiScan');
    if (btnPii) btnPii.style.display = canRedact ? 'inline-flex' : 'none';

    // Signature
    const btnSign = this.$('btnOpenSignModal');
    if (btnSign) btnSign.style.display = canSign ? 'inline-flex' : 'none';

    // Builder
    const btnBuilder = this.$('btnOpenBuilder');
    if (btnBuilder) btnBuilder.style.display = canBuild ? 'inline-flex' : 'none';

    // Forms Save
    const btnSaveForms = this.$('btnSaveForms');
    if (btnSaveForms && isReadOnly) btnSaveForms.style.display = 'none';

    // Search
    const canSearch = this.permissions.canSearch !== false;
    const btnSearch = this.$('btnToggleSearch');
    if (btnSearch) btnSearch.style.display = canSearch ? 'inline-flex' : 'none';

    // Rotation
    const canRotate = this.permissions.canRotate !== false;
    const rotateGroup = this.$('rotateGroup');
    if (rotateGroup) rotateGroup.style.display = canRotate ? 'flex' : 'none';

    // View Modes (Single / Double / Grid)
    const canChangeViewMode = this.permissions.canChangeViewMode !== false;
    const btnLayoutSingle = this.$('btnLayoutSingle');
    const btnLayoutDouble = this.$('btnLayoutDouble');
    const btnLayoutGrid = this.$('btnLayoutGrid');
    if (btnLayoutSingle) btnLayoutSingle.style.display = canChangeViewMode ? 'inline-flex' : 'none';
    if (btnLayoutDouble) btnLayoutDouble.style.display = canChangeViewMode ? 'inline-flex' : 'none';
    if (btnLayoutGrid) btnLayoutGrid.style.display = canChangeViewMode ? 'inline-flex' : 'none';

    // Scroll Modes (Continuous / Paginated)
    const canChangeScrollMode = this.permissions.canChangeScrollMode !== false;
    const btnScrollContinuous = this.$('btnScrollContinuous');
    const btnScrollPage = this.$('btnScrollPage');
    if (btnScrollContinuous) btnScrollContinuous.style.display = canChangeScrollMode ? 'inline-flex' : 'none';
    if (btnScrollPage) btnScrollPage.style.display = canChangeScrollMode ? 'inline-flex' : 'none';

    // Zen Mode
    const canZenMode = this.permissions.canZenMode !== false;
    const btnZenMode = this.$('btnToggleZenMode');
    if (btnZenMode) btnZenMode.style.display = canZenMode ? 'inline-flex' : 'none';
    if (!canZenMode && this.zenMode) {
      this.setZenMode(false);
    }

    // If active tool was hidden, switch to 'select'
    if ((!canAnnotate && (this.currentTool === 'highlight' || this.currentTool === 'note')) ||
        (!canRedact && this.currentTool === 'redact')) {
      this.setTool('select');
    }
  }


  private async checkUrlParams() {
    const urlParams = new URLSearchParams(window.location.search);
    const directUrl = urlParams.get('url');
    const connector = urlParams.get('connector');
    const docId = urlParams.get('id') || urlParams.get('docId');
    const token = urlParams.get('token') || urlParams.get('alf_ticket');

    if (directUrl) {
      this.loadFromUrl(directUrl, token || undefined);
    } else if (connector && docId) {
      this.loadRemote(connector, docId, token || undefined);
    } else if (docId) {
      this.loadDocumentById(docId);
    }
  }

  private setupListeners() {
    // File upload
    const fileInput = this.$('fileUploadInput') as HTMLInputElement;
    this.$('btnUploadDoc')?.addEventListener('click', () => fileInput.click());
    fileInput.addEventListener('change', (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) this.uploadDocument(file);
    });

    // Drag & drop on viewer root + overlay indicator
    // Drag & drop on viewer root + overlay indicator
    const rootEl = (this.root && this.root !== document && 'querySelector' in this.root)
      ? (this.root as HTMLElement)
      : document.body;
    const overlay = this.$('dragDropOverlay');

    let dragCounter = 0;
    const showOverlay = () => {
      if (overlay) overlay.style.display = 'flex';
    };

    const hideOverlay = () => {
      dragCounter = 0;
      if (overlay) overlay.style.display = 'none';
    };

    const handleDragEnter = (e: DragEvent) => {
      e.preventDefault();
      dragCounter++;
      showOverlay();
    };

    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = 'copy';
      }
      showOverlay();
    };

    const handleDragLeave = (e: DragEvent) => {
      e.preventDefault();
      dragCounter = Math.max(0, dragCounter - 1);
      if (dragCounter === 0) {
        if (overlay) overlay.style.display = 'none';
      }
    };

    const handleDrop = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      hideOverlay();
      const file = e.dataTransfer?.files?.[0] || (e.dataTransfer?.items?.[0]?.getAsFile?.());
      if (file) {
        this.dispatchEvent('filedropped', { name: file.name, size: file.size, type: file.type });
        this.uploadDocument(file);
      }
    };

    rootEl.addEventListener('dragenter', handleDragEnter);
    rootEl.addEventListener('dragover', handleDragOver);
    rootEl.addEventListener('dragleave', handleDragLeave);
    rootEl.addEventListener('drop', handleDrop);

    // If viewer is standalone (root is document or body), listen globally on window
    if (rootEl === document.body || !this.root || this.root === document) {
      window.addEventListener('dragenter', handleDragEnter);
      window.addEventListener('dragover', handleDragOver);
      window.addEventListener('dragleave', handleDragLeave);
      window.addEventListener('drop', handleDrop);
    } else {
      // Prevent browser from opening files when dropped outside the viewer component
      window.addEventListener('dragover', (e: DragEvent) => {
        e.preventDefault();
        if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
      });
      window.addEventListener('drop', (e: DragEvent) => {
        e.preventDefault();
      });
    }

    // Page navigation
    this.$('btnPrevPage')?.addEventListener('click', () => this.goToPage(this.activePage - 1));
    this.$('btnNextPage')?.addEventListener('click', () => this.goToPage(this.activePage + 1));
    this.pageNumberInput?.addEventListener('change', () => {
      const p = parseInt(this.pageNumberInput.value, 10);
      if (!isNaN(p)) this.goToPage(p);
    });

    // Viewport scroll page detection
    const viewport = this.$('documentViewport');
    if (viewport) {
      let scrollDebounce: any = null;
      viewport.addEventListener('scroll', () => {
        if (this.viewMode === 'grid' || this.scrollMode === 'page') return;
        if (scrollDebounce) return;
        scrollDebounce = setTimeout(() => {
          scrollDebounce = null;
          if (!this.currentDoc) return;
          const vpRect = viewport.getBoundingClientRect();
          const vpCenterY = vpRect.top + vpRect.height / 2;
          let closestPage = this.activePage;
          let minDistance = Infinity;

          for (const page of this.currentDoc.pages) {
            const el = this.$(`page-${page.page_number}`);
            if (el) {
              const r = el.getBoundingClientRect();
              const elCenterY = r.top + r.height / 2;
              const dist = Math.abs(elCenterY - vpCenterY);
              if (dist < minDistance) {
                minDistance = dist;
                closestPage = page.page_number;
              }
            }
          }

          if (closestPage !== this.activePage) {
            this.activePage = closestPage;
            if (this.pageNumberInput) this.pageNumberInput.value = closestPage.toString();
            this.updateRotateMenuLabels();
            this.updateRotateTooltips();
            this.$$('.thumb-item').forEach((item) => item.classList.remove('active'));
            this.$(`.thumb-item[data-page="${closestPage}"]`)?.classList.add('active');
            this.dispatchEvent('pagechanged', { page: closestPage, total: this.currentDoc.page_count });
          }
        }, 120);
      }, { passive: true });
    }

    // Zoom
    this.$('btnZoomIn')?.addEventListener('click', () => this.setZoom(this.currentZoom + 0.15));
    this.$('btnZoomOut')?.addEventListener('click', () => this.setZoom(this.currentZoom - 0.15));
    this.$('btnFitWidth')?.addEventListener('click', () => this.fitWidth());
    this.$('btnFitPage')?.addEventListener('click', () => this.fitPage());

    // Rotation buttons & Sub-Menu
    this.$('btnRotateCw')?.addEventListener('click', () => this.rotate(90));
    this.$('btnRotateCcw')?.addEventListener('click', () => this.rotate(-90));

    const rotateMenu = this.$('rotateMenu');
    const btnRotateMenu = this.$('btnRotateMenu');
    btnRotateMenu?.addEventListener('click', (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (rotateMenu) {
        const isHidden = rotateMenu.style.display === 'none' || getComputedStyle(rotateMenu).display === 'none';
        rotateMenu.style.display = isHidden ? 'block' : 'none';
        if (isHidden) {
          this.updateRotateMenuLabels();
        }
      }
    });

    const dicomMenu = this.$('menuDicomPresets');
    const btnDicomPresets = this.$('btnDicomPresets');
    btnDicomPresets?.addEventListener('click', (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (dicomMenu) {
        const isHidden = dicomMenu.style.display === 'none' || getComputedStyle(dicomMenu).display === 'none';
        dicomMenu.style.display = isHidden ? 'block' : 'none';
      }
    });

    this.$$('[data-preset]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        e.preventDefault();
        const preset = btn.getAttribute('data-preset');
        if (preset) {
          this.applyDicomPreset(preset);
          if (dicomMenu) dicomMenu.style.display = 'none';
        }
      });
    });

    const btnCine = this.$('btnDicomCinePlay');
    btnCine?.addEventListener('click', (e) => {
      e.stopPropagation();
      e.preventDefault();
      this.toggleDicomCine();
    });

    const closeMenuHandler = (e: Event) => {
      if (rotateMenu && rotateMenu.style.display !== 'none') {
        const targetNode = e.target as Node;
        const isInsideBtn = btnRotateMenu && (btnRotateMenu === targetNode || btnRotateMenu.contains(targetNode));
        const isInsideMenu = rotateMenu.contains(targetNode);
        if (!isInsideBtn && !isInsideMenu) {
          rotateMenu.style.display = 'none';
        }
      }
      if (dicomMenu && dicomMenu.style.display !== 'none') {
        const targetNode = e.target as Node;
        const isInsideBtn = btnDicomPresets && (btnDicomPresets === targetNode || btnDicomPresets.contains(targetNode));
        const isInsideMenu = dicomMenu.contains(targetNode);
        if (!isInsideBtn && !isInsideMenu) {
          dicomMenu.style.display = 'none';
        }
      }
    };
    window.addEventListener('click', closeMenuHandler);
    if (this.root && 'addEventListener' in this.root && this.root !== document) {
      (this.root as HTMLElement).addEventListener('click', closeMenuHandler);
    }

    this.$('menuOptRotateAll')?.addEventListener('click', () => {
      this.setRotateScope('all');
      if (rotateMenu) rotateMenu.style.display = 'none';
    });

    this.$('menuOptRotateCurrent')?.addEventListener('click', () => {
      this.setRotateScope('current');
      if (rotateMenu) rotateMenu.style.display = 'none';
    });

    this.$('menuActionRotatePageCw')?.addEventListener('click', () => {
      this.rotate(90, 'current');
      if (rotateMenu) rotateMenu.style.display = 'none';
    });

    this.$('menuActionRotatePageCcw')?.addEventListener('click', () => {
      this.rotate(-90, 'current');
      if (rotateMenu) rotateMenu.style.display = 'none';
    });

    // Layout Modes & Fullscreen
    this.$('btnLayoutSingle')?.addEventListener('click', () => this.setViewMode('single'));
    this.$('btnLayoutDouble')?.addEventListener('click', () => this.setViewMode('double'));
    this.$('btnLayoutGrid')?.addEventListener('click', () => this.setViewMode('grid'));
    this.$('btnToggleFullscreen')?.addEventListener('click', () => this.toggleFullscreen());
    this.$('btnToggleZenMode')?.addEventListener('click', () => this.toggleZenMode());

    // Scroll Modes (Continu vs Page par page)
    this.$('btnScrollContinuous')?.addEventListener('click', () => this.setScrollMode('continuous'));
    this.$('btnScrollPage')?.addEventListener('click', () => this.setScrollMode('page'));

    // Floating Book Mode Magazine Controls
    this.$('btnBookPrevSpread')?.addEventListener('click', () => {
      if (this.viewMode === 'double') {
        this.prevSpread();
      } else {
        this.goToPage(this.activePage - 1);
      }
    });
    this.$('btnBookNextSpread')?.addEventListener('click', () => {
      if (this.viewMode === 'double') {
        this.nextSpread();
      } else {
        this.goToPage(this.activePage + 1);
      }
    });

    // Keyboard navigation (Ctrl+B for sidebar, F for fullscreen, Z for Zen mode, Left/Right arrows for page turn)
    window.addEventListener('keydown', (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      const isInputActive = activeEl && (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA' || activeEl.getAttribute('contenteditable') === 'true');
      if (!isInputActive) {
        if ((e.ctrlKey || e.metaKey) && (e.key === 'b' || e.key === 'B')) {
          e.preventDefault();
          this.toggleSidebar();
        } else if (e.key === 'f' || e.key === 'F') {
          e.preventDefault();
          this.toggleFullscreen();
        } else if (e.key === 'z' || e.key === 'Z') {
          e.preventDefault();
          this.toggleZenMode();
        } else if (e.key === 'Home') {
          e.preventDefault();
          this.goToPage(1);
        } else if (e.key === 'End') {
          e.preventDefault();
          if (this.currentDoc) {
            this.goToPage(this.currentDoc.page_count);
          }
        } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || e.key === 'PageDown') {
          e.preventDefault();
          if (this.viewMode === 'double') {
            this.nextSpread();
          } else {
            this.goToPage(this.activePage + 1);
          }
        } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || e.key === 'PageUp') {
          e.preventDefault();
          if (this.viewMode === 'double') {
            this.prevSpread();
          } else {
            this.goToPage(this.activePage - 1);
          }
        } else if (e.code === 'Space' && (this.currentDoc?.filename.toLowerCase().endsWith('.dcm') || this.currentDoc?.filename.toLowerCase().endsWith('.dicom'))) {
          e.preventDefault();
          this.toggleDicomCine();
        }
      }
    });

    // Handle fullscreen resize auto-fitting
    document.addEventListener('fullscreenchange', () => {
      setTimeout(() => {
        if (this.viewMode === 'double') {
          this.fitPage();
        } else if (this.viewMode === 'single') {
          this.fitWidth();
        }
      }, 150);
    });

    // VSCode-style Activity Bar & Sidebar tabs
    const tabs = this.$$('.sidebar-tab');
    tabs.forEach((tab) => {
      tab.addEventListener('click', () => {
        const targetTab = tab.getAttribute('data-tab') || 'thumbnails';
        const sidebar = this.$('appSidebar');
        const isCollapsed = !sidebar || sidebar.classList.contains('collapsed');

        // If sidebar is already open on this tab, clicking toggles/collapses it (VSCode UX)
        if (!isCollapsed && this.activeSidebarTab === targetTab) {
          sidebar?.classList.add('collapsed');
          tab.classList.remove('active');
          return;
        }

        // Open sidebar if collapsed
        if (sidebar && isCollapsed) {
          sidebar.classList.remove('collapsed');
        }
        tabs.forEach((t) => t.classList.remove('active'));
        tab.classList.add('active');
        this.activeSidebarTab = targetTab;
        this.renderSidebarContent(targetTab);
      });
    });

    this.$('btnToggleSidebar')?.addEventListener('click', () => {
      this.toggleSidebar();
    });

    this.$('btnCloseSidebar')?.addEventListener('click', () => {
      const sidebar = this.$('appSidebar');
      if (sidebar) {
        sidebar.classList.add('collapsed');
        this.$$('.sidebar-tab').forEach((t) => t.classList.remove('active'));
      }
    });

    // Interactive tools
    const toolBtns = ['toolSelect', 'toolHighlight', 'toolNote', 'toolRedact'];
    toolBtns.forEach((id) => {
      const btn = this.$(id);
      btn?.addEventListener('click', () => {
        toolBtns.forEach((b) => this.$(b)?.classList.remove('active'));
        btn.classList.add('active');
        this.setTool(id.replace('tool', '').toLowerCase());
      });
    });

    // Search overlay
    const searchOverlay = this.$('searchOverlay');
    this.$('btnToggleSearch')?.addEventListener('click', () => {
      if (searchOverlay) {
        searchOverlay.style.display = searchOverlay.style.display === 'none' ? 'flex' : 'none';
        if (searchOverlay.style.display === 'flex') {
          (this.$('searchInput') as HTMLInputElement)?.focus();
        }
      }
    });
    this.$('btnCloseSearch')?.addEventListener('click', () => {
      if (searchOverlay) {
        searchOverlay.style.display = 'none';
        this.clearSearch();
      }
    });
    this.$('searchInput')?.addEventListener('input', (e) => {
      const q = (e.target as HTMLInputElement).value;
      this.searchInDocument(q);
    });
    this.$('btnSearchNext')?.addEventListener('click', () => {
      this.navigateSearch(1);
    });
    this.$('btnSearchPrev')?.addEventListener('click', () => {
      this.navigateSearch(-1);
    });

    // PII Assistant Modal
    const piiModal = this.$('piiModal');
    this.$('btnOpenPiiScan')?.addEventListener('click', () => {
      if (!this.currentDoc) {
        alert('Veuillez ouvrir un document avant de lancer le scan RGPD.');
        return;
      }
      if (piiModal) {
        piiModal.style.display = 'flex';
        this.runPiiScan();
      }
    });
    this.$('btnClosePiiModal')?.addEventListener('click', () => {
      if (piiModal) piiModal.style.display = 'none';
    });
    this.$('btnCancelPiiModal')?.addEventListener('click', () => {
      if (piiModal) piiModal.style.display = 'none';
    });
    this.$('btnApplyPiiRedactions')?.addEventListener('click', () => {
      this.applySelectedPiiRedactions();
    });

    // Signature & Stamp Modal
    const signModal = this.$('signatureModal');
    this.$('btnOpenSignModal')?.addEventListener('click', () => {
      this.openSignatureDialog();
    });
    this.$('btnCloseSignModal')?.addEventListener('click', () => {
      if (signModal) signModal.style.display = 'none';
    });
    this.$('btnCancelSignModal')?.addEventListener('click', () => {
      if (signModal) signModal.style.display = 'none';
    });
    this.$('btnApplySignature')?.addEventListener('click', () => {
      this.applySignature();
    });

    const tabStamp = this.$('tabSigStamp');
    const tabHandwritten = this.$('tabSigHandwritten');
    const stampContent = this.$('sigStampContent');
    const handwrittenContent = this.$('sigHandwrittenContent');

    tabStamp?.addEventListener('click', () => {
      tabStamp.classList.add('active');
      tabHandwritten?.classList.remove('active');
      if (stampContent) stampContent.style.display = 'block';
      if (handwrittenContent) handwrittenContent.style.display = 'none';
    });

    tabHandwritten?.addEventListener('click', () => {
      tabHandwritten.classList.add('active');
      tabStamp?.classList.remove('active');
      if (stampContent) stampContent.style.display = 'none';
      if (handwrittenContent) handwrittenContent.style.display = 'block';
      this.initSignatureCanvas();
    });

    this.$('btnClearCanvas')?.addEventListener('click', () => {
      this.clearSignatureCanvas();
    });


    // Download PDF
    this.$('btnDownloadDoc')?.addEventListener('click', () => {
      this.download();
    });
    this.$('btnPrintDoc')?.addEventListener('click', () => {
      this.print();
    });

    window.addEventListener('keydown', (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
        if (this.permissions.canPrint === false || (this.permissions.readOnly && this.permissions.canPrint !== true)) {
          e.preventDefault();
          e.stopPropagation();
          alert('Impression désactivée par la politique de sécurité (RBAC).');
        }
      }
    });


    this.$('btnSaveForms')?.addEventListener('click', () => {
      this.saveFormValues();
    });

    // Document Builder Modal
    const builderModal = this.$('builderModal');
    this.$('btnOpenBuilder')?.addEventListener('click', () => {
      if (!this.currentDoc) {
        alert('Veuillez ouvrir un document avant de lancer le Builder.');
        return;
      }
      if (builderModal) {
        builderModal.style.display = 'flex';
        const builderGrid = this.$('builderGrid') as HTMLElement;
        const builderUI = new DocumentBuilderUI(this.currentDoc.id, this.currentDoc.pages, (newId) => {
          builderModal.style.display = 'none';
          this.loadDocumentById(newId);
        });
        builderUI.render(builderGrid);

        this.$('btnApplyBuilder')!.onclick = () => {
          const wm = (this.$('builderWatermarkText') as HTMLInputElement).value;
          builderUI.applyBuild(wm);
        };
      }
    });
    this.$('btnCloseBuilder')?.addEventListener('click', () => {
      if (builderModal) builderModal.style.display = 'none';
    });
    this.$('btnCancelBuilder')?.addEventListener('click', () => {
      if (builderModal) builderModal.style.display = 'none';
    });

    // Comparison Modal
    const compareModal = this.$('compareModal');
    this.$('btnOpenCompare')?.addEventListener('click', () => {
      if (!this.currentDoc) {
        alert('Veuillez ouvrir un document actif avant de comparer.');
        return;
      }
      if (compareModal) compareModal.style.display = 'flex';
    });
    this.$('btnCloseCompare')?.addEventListener('click', () => {
      if (compareModal) compareModal.style.display = 'none';
    });
    this.$('btnCancelCompare')?.addEventListener('click', () => {
      if (compareModal) compareModal.style.display = 'none';
    });

    const runComparisonWithFile = (file: File) => {
      if (!this.currentDoc || !compareModal) return;
      const container = this.$('compareResultContainer') as HTMLElement;
      const statsEl = this.$('compareStats') as HTMLElement;
      const compUI = new ComparisonUI(this.currentDoc, compareModal, container, statsEl);
      compUI.runComparison(file);
    };

    this.$('btnRunCompare')?.addEventListener('click', () => {
      if (!this.currentDoc) return;
      const fileInput = this.$('compareFileInput') as HTMLInputElement;
      const file = fileInput.files?.[0];
      if (!file) {
        alert('Veuillez sélectionner un second fichier à comparer.');
        return;
      }
      runComparisonWithFile(file);
    });

    const compareDropZone = this.$('compareResultContainer');
    if (compareDropZone && compareModal) {
      compareDropZone.addEventListener('dragover', (e: DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
        compareDropZone.style.border = '2px dashed var(--accent)';
      });
      compareDropZone.addEventListener('dragleave', (e: DragEvent) => {
        e.preventDefault();
        compareDropZone.style.border = '';
      });
      compareDropZone.addEventListener('drop', (e: DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        compareDropZone.style.border = '';
        const file = e.dataTransfer?.files?.[0] || (e.dataTransfer?.items?.[0]?.getAsFile?.());
        if (file) {
          runComparisonWithFile(file);
        }
      });
    }


    // Redaction Burn-in Execution
    const redactModal = this.$('redactModal');
    this.$('btnConfirmRedact')?.addEventListener('click', async () => {
      if (!this.currentDoc) return;
      const reason = (this.$('redactReasonSelect') as HTMLSelectElement).value;

      // Collect all 'redact' annotations across pages
      const redactionItems: any[] = [];
      for (const ann of this.allAnnotations) {
        if (ann.annotation_type === 'redact') {
          redactionItems.push({
            page_number: ann.page_number,
            x: ann.x,
            y: ann.y,
            width: ann.width,
            height: ann.height,
            reason: reason,
            overlay_text: reason,
          });
        }
      }

      if (redactionItems.length === 0) {
        alert('Aucune zone de biffure sélectionnée sur le document.');
        if (redactModal) redactModal.style.display = 'none';
        return;
      }

      try {
        const resp = await fetch(`/api/documents/${this.currentDoc.id}/redact`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            document_id: this.currentDoc.id,
            items: redactionItems,
          }),
        });

        if (!resp.ok) {
          alert('Erreur lors de la biffure');
          return;
        }

        const newDocMeta = await resp.json();
        alert('Biffure permanente appliquée avec succès ! Chargement du document sécurisé...');
        if (redactModal) redactModal.style.display = 'none';
        this.dispatchEvent('burninapplied', { documentId: newDocMeta.id, redactionsCount: redactionItems.length });
        this.loadDocumentById(newDocMeta.id);
      } catch (e) {
        alert(`Erreur: ${e}`);
      }
    });

    this.$('btnCloseRedact')?.addEventListener('click', () => {
      if (redactModal) redactModal.style.display = 'none';
    });
    this.$('btnCancelRedact')?.addEventListener('click', () => {
      if (redactModal) redactModal.style.display = 'none';
    });
    this.$('btnOpenBurnIn')?.addEventListener('click', () => {
      if (redactModal) redactModal.style.display = 'flex';
    });

    this.setupSelectionToolbar();
  }


  public async uploadDocument(file: File) {
    if (this.isUploading) {
      console.warn('Upload already in progress, skipping duplicate');
      return;
    }
    this.isUploading = true;

    if (this.loadingOverlay) {
      this.loadingOverlay.style.display = 'flex';
      if (this.loadingOverlayTitle) this.loadingOverlayTitle.textContent = `Téléversement : ${file.name}`;
      if (this.loadingOverlaySub) {
        const sizeMb = (file.size / (1024 * 1024)).toFixed(1);
        this.loadingOverlaySub.textContent = `Taille : ${sizeMb} Mo • Traitement et conversion haute fidélité...`;
      }
    }

    this.docTitleEl.textContent = `Chargement de ${file.name}...`;
    this.dispatchEvent('uploadstart', { filename: file.name, size: file.size, type: file.type });

    const formData = new FormData();
    formData.append('file', file);

    try {
      const resp = await fetch('/api/documents', {
        method: 'POST',
        body: formData,
      });

      if (!resp.ok) {
        const errText = await resp.text().catch(() => 'Erreur inconnue');
        throw new Error(errText || 'Erreur lors du chargement du document');
      }

      const meta: DocumentMetadata = await resp.json();
      this.displayDocument(meta);
    } catch (e: any) {
      this.dispatchEvent('uploaderror', { error: e.message || String(e) });
      alert(`Erreur de chargement: ${e.message || e}`);
    } finally {
      if (this.loadingOverlay) {
        this.loadingOverlay.style.display = 'none';
      }
      this.isUploading = false;
    }
  }

  public async loadDocumentById(docId: string) {
    if (this.loadingOverlay) {
      this.loadingOverlay.style.display = 'flex';
      if (this.loadingOverlayTitle) this.loadingOverlayTitle.textContent = 'Chargement du document...';
      if (this.loadingOverlaySub) this.loadingOverlaySub.textContent = 'Récupération des métadonnées et pages...';
    }
    try {
      const resp = await fetch(`/api/documents/${docId}`);
      if (!resp.ok) return;
      const meta: DocumentMetadata = await resp.json();
      this.displayDocument(meta);
    } catch (e) {
      console.error(e);
    } finally {
      if (this.loadingOverlay) {
        this.loadingOverlay.style.display = 'none';
      }
    }
  }

  private displayDocument(meta: DocumentMetadata) {
    this.currentDoc = meta;

    // Determine format badge
    const ext = meta.filename.split('.').pop()?.toLowerCase() || 'pdf';
    let badgeColor = '#ef4444';
    let badgeLabel = 'PDF';

    if (ext === 'docx' || ext === 'doc') {
      badgeColor = '#2563eb';
      badgeLabel = 'WORD';
    } else if (ext === 'xlsx' || ext === 'xls') {
      badgeColor = '#16a34a';
      badgeLabel = 'EXCEL';
    } else if (ext === 'eml' || ext === 'msg') {
      badgeColor = '#9333ea';
      badgeLabel = 'EMAIL';
    } else if (ext === 'txt' || ext === 'csv' || ext === 'log') {
      badgeColor = '#d97706';
      badgeLabel = 'TEXTE';
    } else if (ext === 'dxf' || ext === 'dwg') {
      badgeColor = '#0284c7';
      badgeLabel = 'CAO / DAO';
    } else if (ext === 'dcm' || ext === 'dicom') {
      badgeColor = '#10b981';
      badgeLabel = 'DICOM';
    } else if (meta.mime_type?.startsWith('video/') || ['mp4', 'webm', 'ogv', 'mov', 'mkv', 'avi', 'm4v', '3gp'].includes(ext)) {
      badgeColor = '#ef4444';
      badgeLabel = 'VIDÉO';
    } else if (meta.mime_type?.startsWith('audio/') || ['mp3', 'wav', 'flac', 'aac', 'm4a'].includes(ext)) {
      badgeColor = '#8b5cf6';
      badgeLabel = 'AUDIO';
    }

    this.docTitleEl.innerHTML = `<span style="background: ${badgeColor}; color: white; padding: 2px 7px; border-radius: 4px; font-size: 11px; margin-right: 8px; font-weight: bold;">${this.escapeHtml(badgeLabel)}</span> ${this.escapeHtml(meta.filename)}`;
    this.pageCountLabel.textContent = meta.page_count.toString();
    this.pageNumberInput.value = '1';
    this.pageNumberInput.max = meta.page_count.toString();
    this.activePage = 1;

    // Toggle attachments tab
    const attTab = this.$('attachmentsTab');
    const attBadge = this.$('attachmentsBadge');
    if (attTab) {
      if (meta.attachments && meta.attachments.length > 0) {
        attTab.style.display = 'flex';
        if (attBadge) {
          attBadge.style.display = 'flex';
          attBadge.textContent = meta.attachments.length.toString();
        }
      } else {
        attTab.style.display = 'none';
        if (attBadge) attBadge.style.display = 'none';
      }
    }

    // Toggle CAD technical layers tab
    const cadTab = this.$('cadLayersTab');
    const cadBadge = this.$('cadLayersBadge');
    if (ext === 'dxf' || ext === 'dwg') {
      this.loadCadLayers();
    } else {
      if (cadTab) cadTab.style.display = 'none';
      if (cadBadge) cadBadge.style.display = 'none';
      this.cadMetadata = null;
      this.activeCadLayers.clear();
    }

    // Toggle DICOM medical PACS controls
    const dicomTab = this.$('dicomTab');
    const dicomBadge = this.$('dicomBadge');
    const dicomControls = this.$('dicomControlsGroup');
    const dicomCine = this.$('dicomCineGroup');
    const isDicom = ext === 'dcm' || ext === 'dicom';

    this.stopDicomCine();
    this.dicomCustomWindow = false;

    const rootEl = (this.root && 'querySelector' in this.root && (this.root as HTMLElement).classList.contains('oxid-viewer-root'))
      ? (this.root as HTMLElement)
      : (this.$('.oxid-viewer-root') || document.querySelector('.oxid-viewer-root') as HTMLElement);
    rootEl?.classList.toggle('dicom-mode', isDicom);

    const viewport = this.$('documentViewport');
    viewport?.classList.toggle('dicom-mode', isDicom);

    if (dicomCine) {
      dicomCine.style.display = (isDicom && meta.page_count > 1) ? 'flex' : 'none';
    }

    if (isDicom) {
      this.loadDicomData();
    } else {
      if (dicomTab) dicomTab.style.display = 'none';
      if (dicomBadge) dicomBadge.style.display = 'none';
      if (dicomControls) dicomControls.style.display = 'none';
      this.dicomMetadata = null;
    }

    this.emptyState.style.display = 'none';
    if (viewport) {
      if (!viewport.className.includes('layout-')) {
        viewport.classList.add('layout-single');
      }
      viewport.classList.toggle('scroll-continuous', this.scrollMode === 'continuous');
      viewport.classList.toggle('scroll-page', this.scrollMode === 'page');
    }
    this.pagesContainer.style.display = 'flex';
    this.pagesContainer.innerHTML = '';
    this.textRenderers.clear();
    this.annotationManagers.clear();
    this.pageRotations.clear();
    this.defaultRotation = 0;
    this.rotateScope = 'all';
    this.updateRotateMenuLabels();
    this.updateRotateTooltips();

    // Reset prefetch caches and inform Web Worker
    for (const url of this.cachedPageBlobs.values()) {
      URL.revokeObjectURL(url);
    }
    for (const url of this.cachedThumbBlobs.values()) {
      URL.revokeObjectURL(url);
    }
    this.cachedPageBlobs.clear();
    this.cachedThumbBlobs.clear();
    this.renderWorker?.postMessage({ type: 'CLEAR_CACHE' });

    // Disconnect any existing observer
    if (this.intersectionObserver) {
      this.intersectionObserver.disconnect();
    }
    if (this.thumbIntersectionObserver) {
      this.thumbIntersectionObserver.disconnect();
    }

    this.intersectionObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        const target = entry.target as HTMLElement;
        const pNum = parseInt(target.dataset.pageNumber || '1', 10);
        if (entry.isIntersecting) {
          this.renderPageContent(target, pNum);
        } else if (meta.pages.length > 20) {
          this.unrenderPageContent(target, pNum);
        }
      });
    }, {
      root: this.$('documentViewport'),
      rootMargin: '100% 0px',
    });

    this.thumbIntersectionObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const target = entry.target as HTMLElement;
          const img = target.querySelector('img') as HTMLImageElement;
          const pNum = parseInt(target.getAttribute('data-page') || '0', 10);
          if (img && pNum > 0 && !img.src) {
            const cached = this.cachedThumbBlobs.get(pNum);
            img.src = cached || `/api/documents/${meta.id}/pages/${pNum}/thumbnail`;
          }
          this.thumbIntersectionObserver?.unobserve(target);
        }
      });
    }, {
      root: this.sidebarContent,
      rootMargin: '150px 0px',
    });

    // Multimedia: Video player rendering
    const isVideo = meta.mime_type?.startsWith('video/') || ['mp4', 'webm', 'ogv', 'mov', 'mkv', 'avi', 'm4v', '3gp'].includes(ext);
    if (isVideo) {
      this.renderVideoPlayer(meta);
      this.renderSidebarContent('thumbnails');
      this.dispatchEvent('documentloaded', meta);
      return;
    }

    // Create page nodes
    for (const page of meta.pages) {
      this.createPageElement(page);
    }

    // Immediately render Page 1 without waiting for observer
    const firstPageCard = this.$('page-1');
    if (firstPageCard) {
      this.renderPageContent(firstPageCard, 1);
    }

    // Render sidebar thumbnails (lazy loaded)
    this.renderSidebarContent('thumbnails');

    // Load initial annotations
    this.loadAnnotations();

    // Load interactive form fields
    this.loadFormFields();

    // Trigger predictive prefetching via Web Worker
    this.prefetchNearbyPages(1);

    // Apply page visibility rules according to scrollMode
    this.updateVisiblePages();

    // Emit documentloaded event
    this.dispatchEvent('documentloaded', meta);
  }

  private createPageElement(pageMeta: { page_number: number; width: number; height: number }) {
    const pNum = pageMeta.page_number;
    const ext = this.currentDoc?.filename.split('.').pop()?.toLowerCase() || '';
    const isDicom = ext === 'dcm' || ext === 'dicom';

    const pageCard = document.createElement('div');
    pageCard.className = 'page-container';
    if (isDicom) {
      pageCard.style.backgroundColor = '#000000';
    }
    pageCard.id = `page-${pNum}`;
    pageCard.dataset.pageNumber = pNum.toString();
    pageCard.dataset.rendered = 'false';

    const pageContent = document.createElement('div');
    pageContent.className = 'page-content';
    if (isDicom) {
      pageContent.style.backgroundColor = '#000000';
    }
    pageCard.appendChild(pageContent);

    const placeholder = document.createElement('div');
    placeholder.className = 'page-placeholder';
    placeholder.style.cssText = 'position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: var(--text-muted); font-size: 13px; pointer-events: none;';
    placeholder.textContent = `Page ${pNum} / ${this.currentDoc?.page_count || ''}`;
    pageContent.appendChild(placeholder);

    if (this.watermarkText) {
      this.attachWatermarkOverlay(pageCard, this.watermarkText);
    }
    this.pagesContainer.appendChild(pageCard);
    this.updatePageLayoutDimensions(pNum);
    this.intersectionObserver?.observe(pageCard);
  }

  private renderPageContent(pageCard: HTMLElement, pNum: number) {
    if (pageCard.dataset.rendered === 'true' || !this.currentDoc) return;
    pageCard.dataset.rendered = 'true';

    const pageMeta = this.currentDoc.pages.find((p) => p.page_number === pNum);
    if (!pageMeta) return;

    const baseW = pageMeta.width;
    const baseH = pageMeta.height;
    const scaledW = baseW * this.currentZoom;

    const contentEl = (pageCard.querySelector('.page-content') || pageCard) as HTMLElement;

    const placeholder = contentEl.querySelector('.page-placeholder');
    if (placeholder) placeholder.remove();

    // Page Image (Check worker cache first for instant 120 FPS display)
    let img = contentEl.querySelector('.page-image') as HTMLImageElement;
    let renderUrl = `/api/documents/${this.currentDoc.id}/pages/${pNum}/render?dpi=120`;
    if (this.activeCadLayers.size > 0) {
      renderUrl += `&layers=${encodeURIComponent(Array.from(this.activeCadLayers).join(','))}`;
    }
    if (this.dicomCustomWindow && this.dicomMetadata && typeof this.dicomWc === 'number' && !isNaN(this.dicomWc) && typeof this.dicomWw === 'number' && !isNaN(this.dicomWw)) {
      renderUrl += `&wc=${this.dicomWc}&ww=${this.dicomWw}`;
    }

    const isDynamicDoc = this.activeCadLayers.size > 0 || this.dicomCustomWindow;

    if (!img) {
      img = document.createElement('img');
      img.className = 'page-image';
      img.loading = 'eager';
      img.decoding = 'async';
      const cachedBlob = isDynamicDoc ? null : this.cachedPageBlobs.get(pNum);
      img.src = cachedBlob || renderUrl;
      contentEl.appendChild(img);
    } else {
      const cachedBlob = isDynamicDoc ? null : this.cachedPageBlobs.get(pNum);
      if (cachedBlob && !img.src.startsWith('blob:')) {
        img.src = cachedBlob;
      }
    }

    if (this.dicomMetadata) {
      this.attachPacsOverlay(contentEl, pNum);
    }

    // Trigger prefetch for neighboring pages in background
    setTimeout(() => this.prefetchNearbyPages(pNum), 100);

    // Vector Text Layer (deferred to keep UI thread free for image rendering)
    const loadTextLayer = () => {
      let textLayer = contentEl.querySelector('.text-layer') as HTMLElement;
      if (!textLayer) {
        textLayer = document.createElement('div');
        textLayer.className = 'text-layer';
        contentEl.appendChild(textLayer);
      }

      let textRenderer = this.textRenderers.get(pNum);
      if (!textRenderer) {
        textRenderer = new TextLayerRenderer(textLayer);
        this.textRenderers.set(pNum, textRenderer);
      }
      textRenderer.loadText(this.currentDoc!.id, pNum, baseW, baseH, scaledW);
    };

    if ('requestIdleCallback' in window) {
      (window as any).requestIdleCallback(loadTextLayer, { timeout: 300 });
    } else {
      setTimeout(loadTextLayer, 50);
    }

    // Annotation Layer
    let annotLayer = contentEl.querySelector('.annotation-layer') as HTMLElement;
    if (!annotLayer) {
      annotLayer = document.createElement('div');
      annotLayer.className = 'annotation-layer';
      contentEl.appendChild(annotLayer);
    }

    let annotManager = this.annotationManagers.get(pNum);
    if (!annotManager) {
      annotManager = new AnnotationLayerManager(
        annotLayer,
        pNum,
        (newAnn) => {
          this.allAnnotations.push(newAnn);
          this.saveAnnotations();
          this.updateBurnInButton();
        },
        (deletedId) => {
          this.allAnnotations = this.allAnnotations.filter((a) => a.id !== deletedId);
          this.saveAnnotations();
          this.updateBurnInButton();
        },
        (updatedAnn) => {
          const idx = this.allAnnotations.findIndex((a) => a.id === updatedAnn.id);
          if (idx !== -1) {
            this.allAnnotations[idx] = updatedAnn;
            this.saveAnnotations();
          }
        }
      );
      annotManager.setScale(this.currentZoom);
      annotManager.setTool(this.currentTool);
      annotManager.setAnnotations(this.allAnnotations);
      this.annotationManagers.set(pNum, annotManager);
    }

    // Form Layer
    this.formRenderer.render(contentEl, pNum, this.currentZoom);
  }

  private unrenderPageContent(pageCard: HTMLElement, pNum: number) {
    if (pageCard.dataset.rendered !== 'true') return;
    pageCard.dataset.rendered = 'false';

    const contentEl = (pageCard.querySelector('.page-content') || pageCard) as HTMLElement;
    const img = contentEl.querySelector('.page-image');
    if (img) img.remove();

    if (!contentEl.querySelector('.page-placeholder')) {
      const placeholder = document.createElement('div');
      placeholder.className = 'page-placeholder';
      placeholder.style.cssText = 'position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: var(--text-muted); font-size: 13px; pointer-events: none;';
      placeholder.textContent = `Page ${pNum} / ${this.currentDoc?.page_count || ''}`;
      contentEl.appendChild(placeholder);
    }
  }

  private initSignatureCanvas() {
    this.sigCanvas = this.$('signatureCanvas') as HTMLCanvasElement;
    if (!this.sigCanvas) return;
    this.sigCtx = this.sigCanvas.getContext('2d');
    if (!this.sigCtx) return;

    if (!this.sigCanvas.dataset.initialized) {
      this.sigCanvas.dataset.initialized = 'true';
      this.sigCtx.strokeStyle = '#0f172a';
      this.sigCtx.lineWidth = 2.5;
      this.sigCtx.lineCap = 'round';
      this.sigCtx.lineJoin = 'round';

      const getPos = (e: MouseEvent | Touch) => {
        const rect = this.sigCanvas!.getBoundingClientRect();
        return {
          x: (e.clientX - rect.left) * (this.sigCanvas!.width / rect.width),
          y: (e.clientY - rect.top) * (this.sigCanvas!.height / rect.height),
        };
      };

      const start = (pos: { x: number; y: number }) => {
        this.isDrawing = true;
        this.hasDrawnSignature = true;
        this.sigCtx?.beginPath();
        this.sigCtx?.moveTo(pos.x, pos.y);
      };

      const move = (pos: { x: number; y: number }) => {
        if (!this.isDrawing) return;
        this.sigCtx?.lineTo(pos.x, pos.y);
        this.sigCtx?.stroke();
      };

      const stop = () => {
        this.isDrawing = false;
      };

      this.sigCanvas.addEventListener('mousedown', (e) => start(getPos(e)));
      this.sigCanvas.addEventListener('mousemove', (e) => move(getPos(e)));
      window.addEventListener('mouseup', stop);

      this.sigCanvas.addEventListener('touchstart', (e) => {
        e.preventDefault();
        if (e.touches.length > 0) start(getPos(e.touches[0]));
      }, { passive: false });
      this.sigCanvas.addEventListener('touchmove', (e) => {
        e.preventDefault();
        if (e.touches.length > 0) move(getPos(e.touches[0]));
      }, { passive: false });
      window.addEventListener('touchend', stop);
    }
  }

  private clearSignatureCanvas() {
    if (this.sigCanvas && this.sigCtx) {
      this.sigCtx.clearRect(0, 0, this.sigCanvas.width, this.sigCanvas.height);
      this.hasDrawnSignature = false;
    }
  }

  public openSignatureDialog() {
    if (!this.currentDoc) {
      alert('Veuillez ouvrir un document avant de signer.');
      return;
    }
    const modal = this.$('signatureModal');
    if (modal) {
      modal.style.display = 'flex';
      const pageInput = this.$('sigPageNumber') as HTMLInputElement;
      if (pageInput) {
        pageInput.value = this.activePage.toString();
        pageInput.max = this.currentDoc.page_count.toString();
      }
    }
  }

  public async applySignature() {
    if (!this.currentDoc) return;
    const modal = this.$('signatureModal');
    const signerName = (this.$('sigSignerName') as HTMLInputElement)?.value.trim() || 'Signataire';
    const reason = (this.$('sigReason') as HTMLInputElement)?.value.trim() || 'Approbation légale';
    const location = (this.$('sigLocation') as HTMLInputElement)?.value.trim() || 'Paris, FR';
    const pageNum = parseInt((this.$('sigPageNumber') as HTMLInputElement)?.value || '1', 10);
    const preset = (this.$('sigPositionPreset') as HTMLSelectElement)?.value || 'bottom-right';

    const pageMeta = this.currentDoc.pages.find((p) => p.page_number === pageNum) || this.currentDoc.pages[0];
    const pageW = pageMeta.width;
    const pageH = pageMeta.height;

    const stampW = 250;
    const stampH = 75;
    let stampX = pageW - stampW - 30;
    let stampY = pageH - stampH - 30;

    if (preset === 'bottom-left') {
      stampX = 30;
      stampY = pageH - stampH - 30;
    } else if (preset === 'center') {
      stampX = (pageW - stampW) / 2;
      stampY = (pageH - stampH) / 2;
    }

    let handwrittenB64: string | undefined = undefined;
    const tabHandwritten = this.$('tabSigHandwritten');
    if (tabHandwritten?.classList.contains('active') && this.hasDrawnSignature && this.sigCanvas) {
      handwrittenB64 = this.sigCanvas.toDataURL('image/png');
    }

    const applyBtn = this.$('btnApplySignature') as HTMLButtonElement;
    if (applyBtn) {
      applyBtn.disabled = true;
      applyBtn.textContent = 'Signature cryptographique en cours...';
    }

    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/sign`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          page_number: pageNum,
          x: stampX,
          y: stampY,
          width: stampW,
          height: stampH,
          signer_name: signerName,
          reason,
          location,
          handwritten_png_base64: handwrittenB64,
        }),
      });

      if (resp.ok) {
        const resData = await resp.json();
        if (modal) modal.style.display = 'none';
        await this.loadDocumentById(resData.signed_doc_id);
        setTimeout(() => this.goToPage(pageNum), 200);
      } else {
        alert('Échec de la signature du document.');
      }
    } catch (e) {
      console.error('Signature error', e);
      alert('Erreur réseau lors de la signature.');
    } finally {
      if (applyBtn) {
        applyBtn.disabled = false;
        applyBtn.textContent = 'Certifier et Signer le document';
      }
    }
  }

  public getEffectivePageDimensions(pageNum: number): { width: number; height: number } {
    const pMeta = this.currentDoc?.pages[pageNum - 1] || { width: 595, height: 842 };
    const rot = this.getPageRotation(pageNum);
    const isSideways = Math.abs(rot % 180) === 90;
    return {
      width: isSideways ? pMeta.height : pMeta.width,
      height: isSideways ? pMeta.width : pMeta.height,
    };
  }

  public updatePageLayoutDimensions(pNum: number) {
    if (!this.currentDoc) return;
    const pageMeta = this.currentDoc.pages.find((p) => p.page_number === pNum);
    if (!pageMeta) return;

    const pageCard = this.$(`page-${pNum}`);
    if (!pageCard) return;

    const baseW = pageMeta.width;
    const baseH = pageMeta.height;
    const rot = this.getPageRotation(pNum);
    const isSideways = Math.abs(rot % 180) === 90;

    const scaledW = baseW * this.currentZoom;
    const scaledH = baseH * this.currentZoom;

    const layoutW = isSideways ? scaledH : scaledW;
    const layoutH = isSideways ? scaledW : scaledH;
    const aspectW = isSideways ? baseH : baseW;
    const aspectH = isSideways ? baseW : baseH;

    pageCard.style.width = `${layoutW}px`;
    pageCard.style.height = `${layoutH}px`;
    pageCard.style.aspectRatio = `${aspectW} / ${aspectH}`;
    pageCard.style.setProperty('--page-aspect', `${aspectW} / ${aspectH}`);
    pageCard.style.setProperty('--page-rot', `${rot}deg`);

    // In Grid mode: calculate uniform bounding dimensions so all pages share the exact same visual zoom
    const maxGridDim = 240;
    let gridW = maxGridDim;
    let gridH = maxGridDim;
    if (aspectW >= aspectH) {
      gridW = maxGridDim;
      gridH = Math.max(60, Math.round(maxGridDim * (aspectH / aspectW)));
    } else {
      gridH = maxGridDim;
      gridW = Math.max(60, Math.round(maxGridDim * (aspectW / aspectH)));
    }
    pageCard.style.setProperty('--grid-w', `${gridW}px`);
    pageCard.style.setProperty('--grid-h', `${gridH}px`);

    pageCard.classList.toggle('rotated-sideways', isSideways);
    pageCard.style.transform = '';

    const pageContent = pageCard.querySelector('.page-content') as HTMLElement;
    if (pageContent) {
      pageContent.style.transform = `translate(-50%, -50%) rotate(${rot}deg)`;
      if (this.viewMode !== 'grid') {
        pageContent.style.width = `${scaledW}px`;
        pageContent.style.height = `${scaledH}px`;
      } else {
        pageContent.style.width = '';
        pageContent.style.height = '';
      }
    }
  }

  public setZoom(zoom: number) {
    if (!this.currentDoc) return;
    this.currentZoom = Math.min(Math.max(zoom, 0.3), 3.0);
    this.zoomLevelLabel.textContent = `${Math.round(this.currentZoom * 100)}%`;

    for (const page of this.currentDoc.pages) {
      const pageEl = this.$(`page-${page.page_number}`);
      if (pageEl) {
        this.updatePageLayoutDimensions(page.page_number);

        const textRenderer = this.textRenderers.get(page.page_number);
        textRenderer?.updateScale(this.currentZoom);

        const annotMgr = this.annotationManagers.get(page.page_number);
        annotMgr?.setScale(this.currentZoom);

        if (pageEl.dataset.rendered === 'true') {
          const contentEl = (pageEl.querySelector('.page-content') || pageEl) as HTMLElement;
          this.formRenderer.render(contentEl, page.page_number, this.currentZoom);
        }
      }
    }
  }

  public fitWidth() {
    if (!this.currentDoc || this.currentDoc.pages.length === 0) return;
    const viewportW = this.$('documentViewport')?.clientWidth || 800;

    if (this.viewMode === 'double') {
      const currentOdd = this.activePage % 2 === 1 ? this.activePage : (this.activePage - 1);
      const dims1 = this.getEffectivePageDimensions(currentOdd);
      const hasP2 = (currentOdd + 1) <= this.currentDoc.pages.length;
      const dims2 = hasP2 ? this.getEffectivePageDimensions(currentOdd + 1) : null;
      const totalW = dims2 ? (dims1.width + dims2.width) : (dims1.width * 2);
      const targetZoom = Math.max(0.2, (viewportW - 60) / totalW);
      this.setZoom(targetZoom);
    } else {
      const dims = this.getEffectivePageDimensions(this.activePage);
      const targetZoom = Math.max(0.2, (viewportW - 60) / dims.width);
      this.setZoom(targetZoom);
    }
  }

  public fitPage() {
    if (!this.currentDoc || this.currentDoc.pages.length === 0) return;
    const viewportH = this.$('documentViewport')?.clientHeight || 700;
    const viewportW = this.$('documentViewport')?.clientWidth || 800;

    if (this.viewMode === 'double') {
      const currentOdd = this.activePage % 2 === 1 ? this.activePage : (this.activePage - 1);
      const dims1 = this.getEffectivePageDimensions(currentOdd);
      const hasP2 = (currentOdd + 1) <= this.currentDoc.pages.length;
      const dims2 = hasP2 ? this.getEffectivePageDimensions(currentOdd + 1) : null;
      const totalW = dims2 ? (dims1.width + dims2.width) : (dims1.width * 2);
      const maxH = dims2 ? Math.max(dims1.height, dims2.height) : dims1.height;
      const zoomH = (viewportH - 60) / maxH;
      const zoomW = (viewportW - 60) / totalW;
      this.setZoom(Math.max(0.2, Math.min(zoomH, zoomW)));
    } else {
      const dims = this.getEffectivePageDimensions(this.activePage);
      const zoomH = (viewportH - 60) / dims.height;
      const zoomW = (viewportW - 60) / dims.width;
      this.setZoom(Math.max(0.2, Math.min(zoomH, zoomW)));
    }
  }

  public getPageRotation(pageNum: number): number {
    return this.pageRotations.get(pageNum) ?? this.defaultRotation;
  }

  private applyPageRotation(pNum: number, rot: number) {
    this.updatePageLayoutDimensions(pNum);
    this.updateThumbnailRotation(pNum, rot);
  }

  private updateThumbnailRotation(pNum: number, rot: number) {
    const thumbItem = this.$(`thumb-${pNum}`);
    if (!thumbItem) return;
    const preview = thumbItem.querySelector('.thumb-preview') as HTMLElement;
    const img = preview?.querySelector('img') as HTMLImageElement;
    if (!preview || !img) return;

    const dims = this.getPageThumbDimensions(pNum);
    preview.style.width = `${dims.width}px`;
    preview.style.height = `${dims.height}px`;
    const isSideways = Math.abs(rot % 180) === 90;
    preview.classList.toggle('rotated-sideways', isSideways);
    preview.style.setProperty('--thumb-rot', `${rot}deg`);
    img.style.transform = `translate(-50%, -50%) rotate(${rot}deg)`;
  }

  public setRotateScope(scope: 'all' | 'current') {
    this.rotateScope = scope;
    this.updateRotateMenuLabels();
    this.updateRotateTooltips();
  }

  public getRotateScope(): 'all' | 'current' {
    return this.rotateScope;
  }

  private updateRotateMenuLabels() {
    const pageLabel = this.$('rotateCurrentPageLabel');
    if (pageLabel) {
      pageLabel.textContent = `Page active (P. ${this.activePage})`;
    }
    const actCwLabel = this.$('actionRotateCwLabel');
    if (actCwLabel) {
      actCwLabel.textContent = `Tourner page ${this.activePage} (90° droite)`;
    }
    const actCcwLabel = this.$('actionRotateCcwLabel');
    if (actCcwLabel) {
      actCcwLabel.textContent = `Tourner page ${this.activePage} (90° gauche)`;
    }

    const optAll = this.$('menuOptRotateAll');
    const optCurrent = this.$('menuOptRotateCurrent');
    const checkAll = this.$('checkRotateAll');
    const checkCurrent = this.$('checkRotateCurrent');

    if (this.rotateScope === 'all') {
      optAll?.classList.add('active');
      optCurrent?.classList.remove('active');
      if (checkAll) checkAll.innerHTML = '✓';
      if (checkCurrent) checkCurrent.innerHTML = '&nbsp;';
    } else {
      optCurrent?.classList.add('active');
      optAll?.classList.remove('active');
      if (checkCurrent) checkCurrent.innerHTML = '✓';
      if (checkAll) checkAll.innerHTML = '&nbsp;';
    }
  }

  private updateRotateTooltips() {
    const btnCw = this.$('btnRotateCw');
    const btnCcw = this.$('btnRotateCcw');
    if (this.rotateScope === 'all') {
      if (btnCw) btnCw.title = 'Rotation 90° droite (Tout le document)';
      if (btnCcw) btnCcw.title = 'Rotation 90° gauche (Tout le document)';
    } else {
      if (btnCw) btnCw.title = `Rotation 90° droite (Page ${this.activePage})`;
      if (btnCcw) btnCcw.title = `Rotation 90° gauche (Page ${this.activePage})`;
    }
  }

  public rotate(deg: number, scope?: 'current' | 'all' | number) {
    if (!this.currentDoc) return;

    // Default scope is this.rotateScope (which defaults to 'all' for whole document)
    const targetScope = scope !== undefined ? scope : this.rotateScope;

    if (typeof targetScope === 'number') {
      const pNum = targetScope;
      const current = this.getPageRotation(pNum);
      const newRot = ((current + deg) % 360 + 360) % 360;
      this.pageRotations.set(pNum, newRot);
      this.applyPageRotation(pNum, newRot);
    } else if (targetScope === 'all') {
      this.defaultRotation = ((this.defaultRotation + deg) % 360 + 360) % 360;
      for (const page of this.currentDoc.pages) {
        const pNum = page.page_number;
        const current = this.getPageRotation(pNum);
        const newRot = ((current + deg) % 360 + 360) % 360;
        this.pageRotations.set(pNum, newRot);
        this.applyPageRotation(pNum, newRot);
      }
    } else {
      // 'current' active page
      const pNum = this.activePage;
      const current = this.getPageRotation(pNum);
      const newRot = ((current + deg) % 360 + 360) % 360;
      this.pageRotations.set(pNum, newRot);
      this.applyPageRotation(pNum, newRot);
    }

    if (this.viewMode === 'double') {
      this.fitPage();
    }
    if (this.activeSidebarTab === 'thumbnails') {
      this.renderSidebarContent('thumbnails');
    }

    this.dispatchEvent('pagerotated', {
      scope: targetScope,
      degrees: deg,
      activePage: this.activePage,
      rotations: Object.fromEntries(this.pageRotations.entries()),
    });
  }

  public setTool(tool: string) {
    this.currentTool = tool;
    this.annotationManagers.forEach((mgr) => mgr.setTool(tool));

    // Update toolbar button active states
    const toolBtns = ['toolSelect', 'toolHighlight', 'toolNote', 'toolRedact'];
    toolBtns.forEach((id) => {
      const btn = this.$(id);
      if (btn) {
        const toolName = id.replace('tool', '').toLowerCase();
        if (toolName === tool.toLowerCase()) {
          btn.classList.add('active');
        } else {
          btn.classList.remove('active');
        }
      }
    });

    this.dispatchEvent('toolchanged', { tool });
  }

  public goToPage(pageNum: number) {
    if (!this.currentDoc) return;
    if (pageNum < 1 || pageNum > this.currentDoc.page_count) return;
    this.activePage = pageNum;
    this.pageNumberInput.value = pageNum.toString();
    this.updateRotateMenuLabels();
    this.updateRotateTooltips();

    // Apply visibility rules if in page-by-page mode
    this.updateVisiblePages();

    // Scroll handling: in page mode, reset viewport scroll to top-left to avoid cut-off, otherwise scrollIntoView
    if (this.scrollMode === 'page') {
      const vp = this.$('documentViewport');
      if (vp) {
        vp.scrollTop = 0;
        vp.scrollLeft = 0;
      }
    } else {
      // In double mode, align viewport to the pair (the odd page)
      const targetPageNum = (this.viewMode === 'double' && pageNum % 2 === 0) ? (pageNum - 1) : pageNum;
      const targetEl = this.$(`page-${targetPageNum}`) || this.$(`page-${pageNum}`);
      targetEl?.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'center' });
    }

    // Update thumbnail active state
    this.$$('.thumb-item').forEach((item) => {
      item.classList.remove('active');
    });
    this.$(`.thumb-item[data-page="${pageNum}"]`)?.classList.add('active');

    // Update floating book buttons state
    const prevBtn = this.$('btnBookPrevSpread') as HTMLButtonElement;
    const nextBtn = this.$('btnBookNextSpread') as HTMLButtonElement;
    if (this.viewMode === 'double') {
      if (prevBtn) prevBtn.disabled = pageNum <= 1;
      if (nextBtn) nextBtn.disabled = pageNum >= this.currentDoc.page_count - 1;
    } else {
      if (prevBtn) prevBtn.disabled = pageNum <= 1;
      if (nextBtn) nextBtn.disabled = pageNum >= this.currentDoc.page_count;
    }

    // Update top toolbar navigation buttons
    const btnPrevPage = this.$('btnPrevPage') as HTMLButtonElement;
    const btnNextPage = this.$('btnNextPage') as HTMLButtonElement;
    if (btnPrevPage) btnPrevPage.disabled = pageNum <= 1;
    if (btnNextPage) btnNextPage.disabled = pageNum >= this.currentDoc.page_count;

    this.dispatchEvent('pagechanged', { page: pageNum, total: this.currentDoc.page_count });
  }

  public setScrollMode(mode: 'continuous' | 'page') {
    this.scrollMode = mode;
    const viewport = this.$('documentViewport');
    if (!viewport) return;

    viewport.classList.toggle('scroll-continuous', mode === 'continuous');
    viewport.classList.toggle('scroll-page', mode === 'page');

    const btnContinuous = this.$('btnScrollContinuous');
    const btnPage = this.$('btnScrollPage');
    btnContinuous?.classList.toggle('active', mode === 'continuous');
    btnPage?.classList.toggle('active', mode === 'page');

    this.updateVisiblePages();

    if (mode === 'page') {
      if (this.viewMode === 'double') {
        this.fitPage();
      } else {
        this.fitPage();
      }
    } else {
      // Return to continuous scroll: scroll current active page into view
      const targetPageNum = (this.viewMode === 'double' && this.activePage % 2 === 0) ? (this.activePage - 1) : this.activePage;
      const targetEl = this.$(`page-${targetPageNum}`) || this.$(`page-${this.activePage}`);
      targetEl?.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'center' });
    }

    this.dispatchEvent('scrollmodechanged', { mode });
  }

  public updateVisiblePages() {
    if (!this.currentDoc) return;
    const isPageMode = this.scrollMode === 'page';
    const isDouble = this.viewMode === 'double';
    const isGrid = this.viewMode === 'grid';

    // In grid mode, always show all pages
    if (isGrid || !isPageMode) {
      for (const page of this.currentDoc.pages) {
        const pNum = page.page_number;
        const pageEl = this.$(`page-${pNum}`);
        if (pageEl) {
          pageEl.classList.remove('page-hidden', 'book-page-left', 'book-page-right', 'book-page-single');
          pageEl.style.display = '';
        }
      }
      return;
    }

    // Single page mode (page-by-page): only show this.activePage
    if (!isDouble) {
      for (const page of this.currentDoc.pages) {
        const pNum = page.page_number;
        const pageEl = this.$(`page-${pNum}`);
        if (pageEl) {
          const isCurrent = pNum === this.activePage;
          pageEl.classList.toggle('page-hidden', !isCurrent);
          pageEl.classList.toggle('active-page', isCurrent);
          pageEl.style.display = isCurrent ? '' : 'none';
          if (isCurrent) {
            this.renderPageContent(pageEl, pNum);
          }
        }
      }

      // Pre-render surrounding pages ahead and behind so flipping is 100% instantaneous without any flicker
      const ext = this.currentDoc.filename.split('.').pop()?.toLowerCase() || '';
      const isDicom = ext === 'dcm' || ext === 'dicom';
      const aheadCount = isDicom ? 8 : 4;
      for (let offset = -2; offset <= aheadCount; offset++) {
        const targetP = this.activePage + offset;
        if (targetP >= 1 && targetP <= this.currentDoc.page_count && targetP !== this.activePage) {
          const neighborEl = this.$(`page-${targetP}`);
          if (neighborEl && neighborEl.dataset.rendered !== 'true') {
            this.renderPageContent(neighborEl, targetP);
          }
        }
      }
      return;
    }

    // Double page mode (spread-by-spread)
    const currentOdd = this.activePage % 2 === 1 ? this.activePage : (this.activePage - 1);
    const leftPageNum = currentOdd;
    const rightPageNum = currentOdd + 1;
    const hasRight = rightPageNum <= this.currentDoc.page_count;

    for (const page of this.currentDoc.pages) {
      const pNum = page.page_number;
      const pageEl = this.$(`page-${pNum}`);
      if (pageEl) {
        const isLeft = pNum === leftPageNum;
        const isRight = hasRight && pNum === rightPageNum;
        const isVisible = isLeft || isRight;

        pageEl.classList.toggle('page-hidden', !isVisible);
        pageEl.classList.toggle('active-page', isVisible);
        pageEl.style.display = isVisible ? '' : 'none';

        pageEl.classList.remove('book-page-left', 'book-page-right', 'book-page-single');
        if (isVisible) {
          if (isLeft && hasRight) {
            pageEl.classList.add('book-page-left');
          } else if (isRight) {
            pageEl.classList.add('book-page-right');
          } else {
            pageEl.classList.add('book-page-single');
          }
          this.renderPageContent(pageEl, pNum);
        }
      }
    }
  }

  public setViewMode(mode: 'single' | 'double' | 'grid') {
    this.viewMode = mode;
    const viewport = this.$('documentViewport');
    if (!viewport) return;

    viewport.classList.remove('layout-single', 'layout-double', 'layout-grid');
    viewport.classList.add(`layout-${mode}`);

    // Update active state on layout buttons
    const btnSingle = this.$('btnLayoutSingle');
    const btnDouble = this.$('btnLayoutDouble');
    const btnGrid = this.$('btnLayoutGrid');

    btnSingle?.classList.toggle('active', mode === 'single');
    btnDouble?.classList.toggle('active', mode === 'double');
    btnGrid?.classList.toggle('active', mode === 'grid');

    // Scroll mode buttons: disable/dim them when in grid view
    const btnContinuous = this.$('btnScrollContinuous') as HTMLButtonElement;
    const btnPage = this.$('btnScrollPage') as HTMLButtonElement;
    if (btnContinuous) {
      btnContinuous.disabled = mode === 'grid';
      btnContinuous.style.opacity = mode === 'grid' ? '0.4' : '';
      btnContinuous.style.pointerEvents = mode === 'grid' ? 'none' : '';
    }
    if (btnPage) {
      btnPage.disabled = mode === 'grid';
      btnPage.style.opacity = mode === 'grid' ? '0.4' : '';
      btnPage.style.pointerEvents = mode === 'grid' ? 'none' : '';
    }

    // Adjust zoom & layout behavior
    for (const page of this.currentDoc?.pages || []) {
      this.updatePageLayoutDimensions(page.page_number);
    }

    // Apply visibility changes according to current scroll mode
    this.updateVisiblePages();

    if (mode === 'grid') {
      // In grid mode, enable interactive clicks on pages to jump back to single mode
      this.pagesContainer.querySelectorAll('.page-container').forEach((el) => {
        const pNum = parseInt((el as HTMLElement).dataset.pageNumber || '1', 10);
        (el as HTMLElement).onclick = () => {
          this.setViewMode('single');
          this.goToPage(pNum);
        };
      });
    } else {
      // Reset onclick handlers
      this.pagesContainer.querySelectorAll('.page-container').forEach((el) => {
        (el as HTMLElement).onclick = null;
      });
      if (mode === 'double') {
        requestAnimationFrame(() => {
          this.fitPage();
        });
      } else if (mode === 'single') {
        requestAnimationFrame(() => {
          if (this.scrollMode === 'page') {
            this.fitPage();
          } else {
            this.fitWidth();
          }
        });
      }
    }

    this.dispatchEvent('viewmodechanged', { mode });
  }

  public nextSpread() {
    if (!this.currentDoc) return;
    if (this.viewMode === 'double') {
      // In double mode: if activePage is 1 (or 2), next pair is 3
      const currentOdd = this.activePage % 2 === 1 ? this.activePage : (this.activePage - 1);
      const nextOdd = currentOdd + 2;
      if (nextOdd <= this.currentDoc.page_count) {
        this.goToPage(nextOdd);
      } else if (currentOdd + 1 <= this.currentDoc.page_count) {
        this.goToPage(currentOdd + 1);
      }
    } else {
      this.goToPage(this.activePage + 1);
    }
  }

  public prevSpread() {
    if (!this.currentDoc) return;
    if (this.viewMode === 'double') {
      const currentOdd = this.activePage % 2 === 1 ? this.activePage : (this.activePage - 1);
      const prevOdd = Math.max(1, currentOdd - 2);
      this.goToPage(prevOdd);
    } else {
      this.goToPage(this.activePage - 1);
    }
  }

  public toggleFullscreen() {
    const root = (this.root as HTMLElement) || document.documentElement;
    if (!document.fullscreenElement) {
      if (root.requestFullscreen) {
        root.requestFullscreen();
      } else if ((root as any).webkitRequestFullscreen) {
        (root as any).webkitRequestFullscreen();
      } else if ((root as any).msRequestFullscreen) {
        (root as any).msRequestFullscreen();
      }
    } else {
      if (document.exitFullscreen) {
        document.exitFullscreen();
      } else if ((document as any).webkitExitFullscreen) {
        (document as any).webkitExitFullscreen();
      }
    }
  }

  public isFullscreen(): boolean {
    return !!document.fullscreenElement;
  }

  public toggleZenMode() {
    this.setZenMode(!this.zenMode);
  }

  public setZenMode(enabled: boolean) {
    this.zenMode = enabled;
    const rootEl = (this.root && 'querySelector' in this.root && (this.root as HTMLElement).classList.contains('oxid-viewer-root'))
      ? (this.root as HTMLElement)
      : (this.$('.oxid-viewer-root') || document.querySelector('.oxid-viewer-root') as HTMLElement);

    if (rootEl) {
      rootEl.classList.toggle('zen-mode', enabled);
    }

    const btnZen = this.$('btnToggleZenMode');
    if (btnZen) {
      btnZen.classList.toggle('active', enabled);
      btnZen.setAttribute('title', enabled
        ? 'Désactiver le Mode Zen (Réafficher barres d\'outils fixes) (Z)'
        : 'Mode Zen (Masquer barres d\'outils, réapparition au survol) (Z)');
    }

    // Trigger page readjustment to fill viewport comfortably
    setTimeout(() => {
      if (this.viewMode === 'double') {
        this.fitPage();
      } else if (this.viewMode === 'single') {
        if (this.scrollMode === 'page') {
          this.fitPage();
        } else {
          this.fitWidth();
        }
      }
    }, 150);

    this.dispatchEvent('zenmodechanged', { enabled });
  }

  public openDocumentBuilder() {
    if (!this.currentDoc) {
      alert('Veuillez ouvrir un document avant de lancer le Builder.');
      return;
    }
    const builderModal = this.$('builderModal');
    if (builderModal) {
      builderModal.style.display = 'flex';
      const builderGrid = this.$('builderGrid') as HTMLElement;
      const builderUI = new DocumentBuilderUI(this.currentDoc.id, this.currentDoc.pages, (newId) => {
        builderModal.style.display = 'none';
        this.loadDocumentById(newId);
      });
      builderUI.render(builderGrid);

      const btnApply = this.$('btnApplyBuilder');
      if (btnApply) {
        btnApply.onclick = () => {
          const wm = (this.$('builderWatermarkText') as HTMLInputElement).value;
          builderUI.applyBuild(wm);
        };
      }
    }
  }

  public openComparisonDialog() {
    if (!this.currentDoc) {
      alert('Veuillez ouvrir un document actif avant de comparer.');
      return;
    }
    const compareModal = this.$('compareModal');
    if (compareModal) compareModal.style.display = 'flex';
  }

  public openRedactionDialog() {
    if (!this.currentDoc) {
      alert('Veuillez ouvrir un document.');
      return;
    }
    const redactModal = this.$('redactModal');
    if (redactModal) redactModal.style.display = 'flex';
  }

  public toggleSidebar(tab?: string) {
    const sidebar = this.$('appSidebar');
    if (!sidebar) return;

    if (tab) {
      const isCollapsed = sidebar.classList.contains('collapsed');
      if (!isCollapsed && this.activeSidebarTab === tab) {
        sidebar.classList.add('collapsed');
        this.$$('.sidebar-tab').forEach((t) => t.classList.remove('active'));
        return;
      }
      sidebar.classList.remove('collapsed');
      const tabs = this.$$('.sidebar-tab');
      tabs.forEach((t) => {
        if (t.getAttribute('data-tab') === tab) {
          t.classList.add('active');
        } else {
          t.classList.remove('active');
        }
      });
      this.activeSidebarTab = tab;
      this.renderSidebarContent(tab);
    } else {
      const willCollapse = !sidebar.classList.contains('collapsed');
      sidebar.classList.toggle('collapsed');
      if (willCollapse) {
        this.$$('.sidebar-tab').forEach((t) => t.classList.remove('active'));
      } else {
        const activeTab = this.activeSidebarTab || 'thumbnails';
        this.$$('.sidebar-tab').forEach((t) => {
          t.classList.toggle('active', t.getAttribute('data-tab') === activeTab);
        });
        this.renderSidebarContent(activeTab);
      }
    }
  }

  public getMetadata(): DocumentMetadata | null {
    return this.currentDoc;
  }

  public getAnnotations(): ClientAnnotation[] {
    return this.allAnnotations;
  }

  private renderSidebarContent(tab: string) {
    if (!this.currentDoc) return;
    this.sidebarContent.innerHTML = '';
    this.sidebarContent.onscroll = null;

    // Update panel header title
    const titleEl = this.$('sidebarPanelTitle');
    if (titleEl) {
      const titles: Record<string, string> = {
        thumbnails: 'VIGNETTES',
        bookmarks: 'SIGNETS',
        annotations: `ANNOTATIONS${this.allAnnotations.length > 0 ? ` (${this.allAnnotations.length})` : ''}`,
        forms: `FORMULAIRE${this.currentForms?.fields_count ? ` (${this.currentForms.fields_count})` : ''}`,
        attachments: `PIÈCES JOINTES${this.currentDoc.attachments?.length ? ` (${this.currentDoc.attachments.length})` : ''}`,
        'cad-layers': `CALQUES CAO / DAO${this.cadMetadata?.layers ? ` (${this.cadMetadata.layers.length})` : ''}`,
        dicom: `IMAGERIE MÉDICALE DICOM${this.dicomMetadata?.modality ? ` (${this.dicomMetadata.modality})` : ''}`,
        info: 'PROPRIÉTÉS DU DOCUMENT',
      };
      titleEl.textContent = titles[tab] || tab.toUpperCase();
    }

    if (tab === 'thumbnails') {
      const ext = this.currentDoc.filename.split('.').pop()?.toLowerCase() || '';
      const isVideo = this.currentDoc.mime_type?.startsWith('video/') || ['mp4', 'webm', 'ogv', 'mov', 'mkv', 'avi', 'm4v', '3gp'].includes(ext);
      if (isVideo) {
        this.sidebarContent.innerHTML = `
          <div class="thumb-item active" style="position: relative; cursor: pointer; border-radius: 6px; overflow: hidden; border: 2px solid var(--accent); background: #000; padding: 4px;">
            <img src="/api/documents/${this.currentDoc.id}/pages/1/render?dpi=100" style="width: 100%; border-radius: 4px; height: auto; display: block;" onerror="this.style.display='none'">
            <div class="video-badge-thumb">▶ VIDÉO</div>
          </div>
          <div style="font-size: 11px; color: var(--text-muted); text-align: center; margin-top: 8px; word-break: break-all;">
            ${this.currentDoc.filename}
          </div>
        `;
        return;
      }

      const totalPages = this.currentDoc.page_count;

      if (totalPages <= 15) {
        for (const page of this.currentDoc.pages) {
          this.sidebarContent.appendChild(this.createThumbnailItem(page.page_number, false));
        }
      } else {
        // Dynamic cumulative offsets for smooth virtualization with mixed aspect ratios (Landscape/Portrait)
        const offsets: number[] = [];
        let totalH = 0;
        for (let i = 0; i < totalPages; i++) {
          offsets.push(totalH);
          totalH += this.getPageThumbDimensions(i + 1).itemHeight;
        }

        const virtualContainer = document.createElement('div');
        virtualContainer.className = 'thumb-virtual-container';
        virtualContainer.style.height = `${totalH}px`;
        this.sidebarContent.appendChild(virtualContainer);

        let ticking = false;
        this.sidebarContent.onscroll = () => {
          if (!ticking) {
            requestAnimationFrame(() => {
              this.updateVirtualizedThumbnails(virtualContainer, offsets);
              ticking = false;
            });
            ticking = true;
          }
        };
        this.updateVirtualizedThumbnails(virtualContainer, offsets);
      }
    } else if (tab === 'bookmarks') {
      if (this.currentDoc.bookmarks.length === 0) {
        this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Aucun signet dans ce document.</p>';
      } else {
        for (const bm of this.currentDoc.bookmarks) {
          const el = document.createElement('div');
          el.style.padding = '8px 4px';
          el.style.fontSize = '13px';
          el.style.cursor = 'pointer';
          el.textContent = `📑 ${bm.title}`;
          el.onclick = () => this.goToPage(bm.page_number);
          this.sidebarContent.appendChild(el);
        }
      }
    } else if (tab === 'annotations') {
      if (this.allAnnotations.length === 0) {
        this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Aucune annotation.</p>';
      } else {
        for (const ann of this.allAnnotations) {
          const el = document.createElement('div');
          el.style.padding = '8px';
          el.style.marginBottom = '6px';
          el.style.background = 'var(--bg-primary)';
          el.style.borderRadius = '4px';
          el.style.fontSize = '12px';
          const safeContent = this.escapeHtml(ann.content || ann.reason || 'Surligné');
          const safeType = this.escapeHtml(ann.annotation_type.toUpperCase());
          el.innerHTML = `<strong>P.${ann.page_number} [${safeType}]</strong> - ${safeContent}`;
          el.onclick = () => this.goToPage(ann.page_number);
          this.sidebarContent.appendChild(el);
        }
      }
    } else if (tab === 'attachments') {
      if (!this.currentDoc.attachments || this.currentDoc.attachments.length === 0) {
        this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Aucune pièce jointe.</p>';
      } else {
        for (const att of this.currentDoc.attachments) {
          const el = document.createElement('div');
          el.style.padding = '10px';
          el.style.marginBottom = '8px';
          el.style.background = 'var(--bg-primary)';
          el.style.borderRadius = '6px';
          el.style.border = '1px solid var(--border)';
          el.style.display = 'flex';
          el.style.alignItems = 'center';
          el.style.justifyContent = 'space-between';
          el.style.fontSize = '12px';

          const sizeKb = (att.size / 1024).toFixed(1);
          el.innerHTML = `
            <div style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 160px;">
              <strong>📎 ${this.escapeHtml(att.filename)}</strong><br>
              <span style="color: var(--text-muted); font-size: 11px;">${sizeKb} Ko (${this.escapeHtml(att.mime_type)})</span>
            </div>
          `;
          this.sidebarContent.appendChild(el);
        }
      }
    } else if (tab === 'forms') {
      const fields = this.formRenderer.getFields();
      if (fields.length === 0) {
        this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Aucun champ de formulaire interactif détecté.</p>';
      } else {
        const header = document.createElement('div');
        header.style.cssText = 'padding: 8px 4px; font-size: 12px; color: var(--text-muted); font-weight: 600; text-transform: uppercase; border-bottom: 1px solid var(--border); margin-bottom: 8px;';
        header.textContent = `${fields.length} champs détectés`;
        this.sidebarContent.appendChild(header);

        for (const field of fields) {
          const el = document.createElement('div');
          el.style.cssText = 'padding: 10px; margin-bottom: 8px; background: var(--bg-primary); border-radius: 6px; border: 1px solid var(--border); font-size: 12px; display: flex; flex-direction: column; gap: 4px; cursor: pointer; transition: border-color 0.15s;';
          const safeName = this.escapeHtml(field.name);
          const safeType = this.escapeHtml(field.field_type);
          const safeVal = this.escapeHtml(this.formRenderer.getValue(field.name) || '(vide)');
          el.innerHTML = `
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <strong style="color: var(--accent);">${safeName}</strong>
              <span style="font-size: 10px; background: var(--bg-secondary); padding: 2px 5px; border-radius: 4px; border: 1px solid var(--border);">${safeType}</span>
            </div>
            <div style="color: var(--text-muted); font-size: 11px;">Page ${field.page_number} ${field.required ? '• <span style="color:#ef4444;">Requis</span>' : ''}</div>
            <div style="font-size: 12px; color: var(--text-main); word-break: break-all;">Valeur: <span style="font-family: monospace; background: var(--bg-secondary); padding: 1px 4px; border-radius: 3px;">${safeVal}</span></div>
          `;
          el.onclick = () => {
            this.goToPage(field.page_number);
            setTimeout(() => {
              const input = document.querySelector(`[data-field-name="${field.name}"]`) as HTMLElement;
              if (input) {
                input.focus();
                input.scrollIntoView({ behavior: 'smooth', block: 'center' });
              }
            }, 300);
          };
          this.sidebarContent.appendChild(el);
        }
      }
    } else if (tab === 'info') {
      const doc = this.currentDoc;
      const sizeMb = (doc.file_size / (1024 * 1024)).toFixed(2);
      const sizeKb = (doc.file_size / 1024).toFixed(1);
      const sizeDisplay = doc.file_size > 1024 * 1024 ? `${sizeMb} Mo (${doc.file_size.toLocaleString()} octets)` : `${sizeKb} Ko`;
      const firstPage = doc.pages[0];
      const dims = firstPage ? `${firstPage.width} × ${firstPage.height} pt` : 'Inconnu';
      const rbacPerms = this.permissions;

      const infoContainer = document.createElement('div');
      infoContainer.style.cssText = 'display: flex; flex-direction: column; gap: 12px; font-size: 12px;';

      const sectionDoc = document.createElement('div');
      sectionDoc.style.cssText = 'background: var(--bg-primary); border: 1px solid var(--border); border-radius: 6px; padding: 12px; display: flex; flex-direction: column; gap: 8px;';
      sectionDoc.innerHTML = `
        <div style="font-weight: 700; color: var(--accent); text-transform: uppercase; font-size: 11px; letter-spacing: 0.5px; border-bottom: 1px solid var(--border); padding-bottom: 4px; margin-bottom: 4px;">
          📄 Document
        </div>
        <div style="display: flex; justify-content: space-between; word-break: break-all;">
          <span style="color: var(--text-muted);">Nom :</span>
          <strong style="text-align: right; max-width: 150px;">${this.escapeHtml(doc.filename)}</strong>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Pages :</span>
          <strong>${doc.page_count}</strong>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Taille :</span>
          <span>${sizeDisplay}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Type MIME :</span>
          <span>${this.escapeHtml(doc.mime_type)}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Dimensions P.1 :</span>
          <span>${dims}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Signets :</span>
          <span>${doc.bookmarks.length}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Pièces jointes :</span>
          <span>${doc.attachments?.length || 0}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Annotations :</span>
          <span>${this.allAnnotations.length}</span>
        </div>
      `;
      infoContainer.appendChild(sectionDoc);

      const sectionRbac = document.createElement('div');
      sectionRbac.style.cssText = 'background: var(--bg-primary); border: 1px solid var(--border); border-radius: 6px; padding: 12px; display: flex; flex-direction: column; gap: 8px;';
      sectionRbac.innerHTML = `
        <div style="font-weight: 700; color: #f59e0b; text-transform: uppercase; font-size: 11px; letter-spacing: 0.5px; border-bottom: 1px solid var(--border); padding-bottom: 4px; margin-bottom: 4px;">
          🛡️ Sécurité & Droits RBAC
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Téléchargement :</span>
          <span>${rbacPerms.canDownload !== false && !rbacPerms.readOnly ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Impression :</span>
          <span>${rbacPerms.canPrint !== false && !rbacPerms.readOnly ? '✅ Autorisée' : '❌ Bloquée'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Biffure / RGPD :</span>
          <span>${rbacPerms.canRedact !== false && !rbacPerms.readOnly ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Signature :</span>
          <span>${rbacPerms.canSign !== false && !rbacPerms.readOnly ? '✅ Autorisée' : '❌ Bloquée'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Builder & Fusion :</span>
          <span>${rbacPerms.canBuild !== false && !rbacPerms.readOnly ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Rotation des pages :</span>
          <span>${rbacPerms.canRotate !== false ? '✅ Autorisée' : '❌ Bloquée'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Modes d'affichage :</span>
          <span>${rbacPerms.canChangeViewMode !== false ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Modes de défilement :</span>
          <span>${rbacPerms.canChangeScrollMode !== false ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Mode Zen :</span>
          <span>${rbacPerms.canZenMode !== false ? '✅ Autorisé' : '❌ Bloqué'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Mode Lecture seule :</span>
          <span>${rbacPerms.readOnly ? '🔴 Oui' : '🟢 Non'}</span>
        </div>
        <div style="display: flex; justify-content: space-between;">
          <span style="color: var(--text-muted);">Filigrane dynamique :</span>
          <span>${this.watermarkText ? '🛡️ Actif' : 'Aucun'}</span>
        </div>
      `;
      infoContainer.appendChild(sectionRbac);

      this.sidebarContent.appendChild(infoContainer);
    } else if (tab === 'cad-layers') {
      this.renderCadLayersContent();
    } else if (tab === 'dicom') {
      this.renderDicomContent();
    }
  }

  // =========================================================================
  // CAD / DAO Plans & Technical Layers Management
  // =========================================================================

  public async loadCadLayers() {
    if (!this.currentDoc) return;
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/cad/layers`);
      if (resp.ok) {
        const data = await resp.json();
        const layers = Array.isArray(data) ? data : (data.layers || []);
        this.cadMetadata = { layers };
        this.activeCadLayers = new Set(layers.map((l: any) => l.name));
        const cadTab = this.$('cadLayersTab');
        const cadBadge = this.$('cadLayersBadge');
        if (cadTab) cadTab.style.display = 'flex';
        if (cadBadge) {
          cadBadge.style.display = 'flex';
          cadBadge.textContent = layers.length.toString();
        }
        if (this.activeSidebarTab === 'cad-layers') {
          this.renderSidebarContent('cad-layers');
        }
      }
    } catch (e) {
      console.warn('Failed to load CAD layers:', e);
    }
  }

  public getCadLayers(): any[] {
    return this.cadMetadata?.layers || [];
  }

  public toggleCadLayer(layerName: string, visible?: boolean) {
    const isNowVisible = visible !== undefined ? visible : !this.activeCadLayers.has(layerName);
    if (isNowVisible) {
      this.activeCadLayers.add(layerName);
    } else {
      this.activeCadLayers.delete(layerName);
    }
    this.refreshCadRendering();
    if (this.activeSidebarTab === 'cad-layers') {
      this.renderSidebarContent('cad-layers');
    }
    this.dispatchEvent('cadlayerschange', { layers: Array.from(this.activeCadLayers) });
  }

  public setCadLayers(layers: string[]) {
    this.activeCadLayers = new Set(layers);
    this.refreshCadRendering();
    if (this.activeSidebarTab === 'cad-layers') {
      this.renderSidebarContent('cad-layers');
    }
    this.dispatchEvent('cadlayerschange', { layers });
  }

  private refreshCadRendering() {
    if (!this.currentDoc) return;
    const layerParam = encodeURIComponent(Array.from(this.activeCadLayers).join(','));
    for (const page of this.currentDoc.pages) {
      const pNum = page.page_number;
      const card = this.$(`page-${pNum}`);
      if (card && card.dataset.rendered === 'true') {
        const img = card.querySelector('.page-image') as HTMLImageElement;
        if (img) {
          img.src = `/api/documents/${this.currentDoc.id}/pages/${pNum}/render?dpi=120&layers=${layerParam}&t=${Date.now()}`;
        }
      }
    }
  }

  private renderCadLayersContent() {
    if (!this.cadMetadata || !this.cadMetadata.layers) {
      this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Chargement des calques CAO / DAO...</p>';
      return;
    }

    const layers = this.cadMetadata.layers;
    const totalEntities = layers.reduce((acc: number, l: any) => acc + (l.entity_count || 0), 0);

    const container = document.createElement('div');
    container.className = 'cad-panel';

    container.innerHTML = `
      <div style="font-size: 11px; color: var(--text-muted); padding-bottom: 4px; border-bottom: 1px solid var(--border);">
        <strong>${layers.length}</strong> calques • <strong>${totalEntities}</strong> entités vectorielles
      </div>
      <div class="cad-panel-toolbar">
        <input type="text" class="cad-search-input" id="cadLayerFilter" placeholder="Filtrer les calques...">
        <button class="cad-btn-action" id="btnCadSelectAll" title="Afficher tous les calques">Tous</button>
        <button class="cad-btn-action" id="btnCadDeselectAll" title="Masquer tous les calques">Aucun</button>
      </div>
      <div class="cad-layers-list" id="cadLayersList"></div>
    `;

    const listEl = container.querySelector('#cadLayersList') as HTMLElement;

    const renderList = (filter = '') => {
      listEl.innerHTML = '';
      const filterLower = filter.toLowerCase();
      const filtered = layers.filter((l: any) => l.name.toLowerCase().includes(filterLower));

      if (filtered.length === 0) {
        listEl.innerHTML = '<div style="color: var(--text-muted); font-size: 11px; padding: 12px; text-align: center;">Aucun calque trouvé</div>';
        return;
      }

      for (const layer of filtered) {
        const item = document.createElement('div');
        const isActive = this.activeCadLayers.has(layer.name);
        item.className = `cad-layer-item ${isActive ? '' : 'disabled'}`;
        item.innerHTML = `
          <input type="checkbox" class="cad-layer-checkbox" ${isActive ? 'checked' : ''}>
          <div class="cad-layer-color" style="background-color: ${layer.color_hex};"></div>
          <span class="cad-layer-name" title="${layer.name}">${layer.name}</span>
          <span class="cad-layer-count" title="${layer.entity_count} éléments">${layer.entity_count}</span>
        `;

        const cb = item.querySelector('.cad-layer-checkbox') as HTMLInputElement;
        const toggle = (e: Event) => {
          e.stopPropagation();
          this.toggleCadLayer(layer.name);
        };
        cb.addEventListener('change', toggle);
        item.addEventListener('click', (e) => {
          if (e.target !== cb) {
            cb.checked = !cb.checked;
            this.toggleCadLayer(layer.name);
          }
        });

        listEl.appendChild(item);
      }
    };

    renderList();

    const searchInput = container.querySelector('#cadLayerFilter') as HTMLInputElement;
    searchInput?.addEventListener('input', () => {
      renderList(searchInput.value.trim());
    });

    container.querySelector('#btnCadSelectAll')?.addEventListener('click', () => {
      this.setCadLayers(layers.map((l: any) => l.name));
    });

    container.querySelector('#btnCadDeselectAll')?.addEventListener('click', () => {
      this.setCadLayers([]);
    });

    this.sidebarContent.appendChild(container);
  }

  // =========================================================================
  // DICOM Medical Imaging & Contrast Windowing (HU)
  // =========================================================================

  public async loadDicomData() {
    if (!this.currentDoc) return;
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/dicom/metadata`);
      if (resp.ok) {
        this.dicomMetadata = await resp.json();
        const defWc = this.dicomMetadata.default_window_center ?? this.dicomMetadata.window_center ?? 40;
        const defWw = this.dicomMetadata.default_window_width ?? this.dicomMetadata.window_width ?? 400;
        this.dicomWc = typeof defWc === 'number' && !isNaN(defWc) ? defWc : 40;
        this.dicomWw = typeof defWw === 'number' && !isNaN(defWw) ? defWw : 400;

        const dicomTab = this.$('dicomTab');
        const dicomBadge = this.$('dicomBadge');
        const dicomControls = this.$('dicomControlsGroup');
        if (dicomTab) dicomTab.style.display = 'flex';
        if (dicomBadge) {
          dicomBadge.style.display = 'flex';
          dicomBadge.textContent = this.dicomMetadata.modality || 'CT';
        }
        if (dicomControls) dicomControls.style.display = 'flex';

        // Attach overlay to existing rendered pages
        for (const page of this.currentDoc.pages) {
          const card = this.$(`page-${page.page_number}`);
          if (card) {
            const content = card.querySelector('.page-content') || card;
            this.attachPacsOverlay(content as HTMLElement, page.page_number);
          }
        }

        if (this.activeSidebarTab === 'dicom') {
          this.renderSidebarContent('dicom');
        }
      }
    } catch (e) {
      console.warn('Failed to load DICOM metadata:', e);
    }
  }

  public getDicomMetadata(): any | null {
    return this.dicomMetadata;
  }

  public applyDicomPreset(presetName: string) {
    const presets: Record<string, { wc: number; ww: number; label: string }> = {
      soft_tissue: { wc: 40, ww: 400, label: 'Tissus mous' },
      lung: { wc: -600, ww: 1500, label: 'Poumons' },
      bone: { wc: 400, ww: 1800, label: 'Os / Squelette' },
      brain: { wc: 40, ww: 80, label: 'Cerveau / AVC' },
      mediastinum: { wc: 50, ww: 350, label: 'Médiastin' },
      default: {
        wc: this.dicomMetadata?.default_window_center ?? this.dicomMetadata?.window_center ?? 40,
        ww: this.dicomMetadata?.default_window_width ?? this.dicomMetadata?.window_width ?? 400,
        label: 'Défaut',
      },
    };
    const target = presets[presetName] || presets.default;
    const labelEl = this.$('dicomPresetLabel');
    if (labelEl) labelEl.textContent = target.label;

    // Update active class & checkmarks in toolbar dropdown
    this.$$('[data-preset]').forEach((b) => {
      const isCurrent = b.getAttribute('data-preset') === presetName;
      b.classList.toggle('active', isCurrent);
      const check = b.querySelector('.dropdown-check');
      if (check) check.innerHTML = isCurrent ? '✓' : '&nbsp;';
    });

    // Update active class in sidebar grid
    this.$$('[data-preset-name]').forEach((b) => {
      b.classList.toggle('active', b.getAttribute('data-preset-name') === presetName);
    });

    this.setDicomWindow(target.wc, target.ww);
  }

  public setDicomWindow(center: number, width: number) {
    this.dicomCustomWindow = true;
    this.dicomWc = center;
    this.dicomWw = Math.max(1, width);

    // Update PACS HUD overlays
    this.$$('.pacs-wc-ww').forEach((el) => {
      el.textContent = `WC: ${this.dicomWc} WW: ${this.dicomWw}`;
    });

    // Update sidebar inputs/display if open
    const sliderWc = this.$('dicomSliderWc') as HTMLInputElement;
    const sliderWw = this.$('dicomSliderWw') as HTMLInputElement;
    const valWc = this.$('dicomValWc');
    const valWw = this.$('dicomValWw');
    const huRange = this.$('dicomHuRange');
    if (sliderWc) sliderWc.value = this.dicomWc.toString();
    if (sliderWw) sliderWw.value = this.dicomWw.toString();
    if (valWc) valWc.textContent = `${this.dicomWc} HU`;
    if (valWw) valWw.textContent = `${this.dicomWw} HU`;
    if (huRange) {
      const minHu = Math.round(this.dicomWc - this.dicomWw / 2);
      const maxHu = Math.round(this.dicomWc + this.dicomWw / 2);
      huRange.textContent = `Plage : [${minHu} HU → ${maxHu} HU]`;
    }

    // Debounce image re-rendering
    if (this.dicomWindowingTimeout) {
      clearTimeout(this.dicomWindowingTimeout);
    }
    this.dicomWindowingTimeout = setTimeout(() => {
      this.refreshDicomRendering();
      this.dispatchEvent('dicomwindowchange', { wc: this.dicomWc, ww: this.dicomWw });
    }, 40);
  }

  private refreshDicomRendering() {
    if (!this.currentDoc) return;
    for (const page of this.currentDoc.pages) {
      const pNum = page.page_number;
      const card = this.$(`page-${pNum}`);
      if (card && card.dataset.rendered === 'true') {
        const img = card.querySelector('.page-image') as HTMLImageElement;
        if (img) {
          if (typeof this.dicomWc === 'number' && !isNaN(this.dicomWc) && typeof this.dicomWw === 'number' && !isNaN(this.dicomWw)) {
            img.src = `/api/documents/${this.currentDoc.id}/pages/${pNum}/render?dpi=120&wc=${this.dicomWc}&ww=${this.dicomWw}&t=${Date.now()}`;
          } else {
            img.src = `/api/documents/${this.currentDoc.id}/pages/${pNum}/render?dpi=120&t=${Date.now()}`;
          }
        }
      }
    }
  }

  public toggleDicomCine() {
    if (this.dicomCineInterval) {
      this.stopDicomCine();
    } else {
      if (!this.currentDoc || this.currentDoc.page_count <= 1) return;
      this.updateCineBtn(true);
      this.dicomCineInterval = setInterval(() => {
        if (!this.currentDoc) return;
        const next = (this.activePage % this.currentDoc.page_count) + 1;
        this.goToPage(next);
      }, 66); // ~15 FPS cine loop rate
    }
  }

  public stopDicomCine() {
    if (this.dicomCineInterval) {
      clearInterval(this.dicomCineInterval);
      this.dicomCineInterval = null;
      this.updateCineBtn(false);
    }
  }

  private updateCineBtn(isPlaying: boolean) {
    const btn = this.$('btnDicomCinePlay');
    if (!btn) return;
    btn.classList.toggle('btn-cine-playing', isPlaying);
    btn.innerHTML = isPlaying
      ? `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>`
      : `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>`;
  }

  private attachPacsOverlay(container: HTMLElement, pNum: number) {
    let overlay = container.querySelector('.pacs-overlay') as HTMLElement;
    if (!overlay) {
      overlay = document.createElement('div');
      overlay.className = 'pacs-overlay';
      container.appendChild(overlay);
    }
    const meta = this.dicomMetadata || {};
    const patientName = meta.patient_name || 'ANONYMOUS';
    const patientId = meta.patient_id || 'N/A';
    const modality = meta.modality || 'CT';
    const studyDate = meta.study_date || '';
    const zoomPct = Math.round(this.currentZoom * 100);
    const rot = this.getPageRotation(pNum);

    overlay.innerHTML = `
      <div class="pacs-overlay-top">
        <div class="pacs-corner">
          <span>${patientName}</span>
          <span>ID: ${patientId}</span>
        </div>
        <div class="pacs-corner pacs-corner-right">
          <span>${studyDate}</span>
          <span>MOD: ${modality}</span>
        </div>
      </div>
      <div class="pacs-overlay-bottom">
        <div class="pacs-corner">
          <span>ZOOM: ${zoomPct}%</span>
          <span>ROT: ${rot}°</span>
        </div>
        <div class="pacs-corner pacs-corner-right">
          <span class="pacs-wc-ww">WC: ${this.dicomWc} WW: ${this.dicomWw}</span>
          <span>HU: VOI LUT</span>
        </div>
      </div>
    `;
  }

  private renderDicomContent() {
    if (!this.dicomMetadata) {
      this.sidebarContent.innerHTML = '<p style="color: var(--text-muted); font-size: 13px;">Chargement des données DICOM...</p>';
      return;
    }

    const meta = this.dicomMetadata;
    const minHu = Math.round(this.dicomWc - this.dicomWw / 2);
    const maxHu = Math.round(this.dicomWc + this.dicomWw / 2);

    const container = document.createElement('div');
    container.className = 'dicom-panel';

    container.innerHTML = `
      <!-- Section Informations Patient / PACS -->
      <div class="dicom-section">
        <div class="dicom-section-title">
          <span>🩺 Examen Médical</span>
          <span style="font-size: 10px; background: rgba(16, 185, 129, 0.2); padding: 1px 6px; border-radius: 4px;">${meta.modality || 'CT'}</span>
        </div>
        <div class="dicom-meta-row">
          <span class="dicom-meta-label">Patient :</span>
          <span class="dicom-meta-value">${meta.patient_name || 'Anonymisé'}</span>
        </div>
        <div class="dicom-meta-row">
          <span class="dicom-meta-label">ID Patient :</span>
          <span class="dicom-meta-value">${meta.patient_id || 'N/A'}</span>
        </div>
        <div class="dicom-meta-row">
          <span class="dicom-meta-label">Date :</span>
          <span class="dicom-meta-value">${meta.study_date || 'N/A'}</span>
        </div>
        <div class="dicom-meta-row">
          <span class="dicom-meta-label">Matrice :</span>
          <span class="dicom-meta-value">${meta.columns} × ${meta.rows} px</span>
        </div>
        <div class="dicom-meta-row">
          <span class="dicom-meta-label">Bits alloués :</span>
          <span class="dicom-meta-value">${meta.bits_allocated} bits (${meta.bits_stored} bits stockés)</span>
        </div>
      </div>

      <!-- Section Préréglages Hounsfield -->
      <div class="dicom-section">
        <div class="dicom-section-title">
          <span>🎯 Préréglages de Contraste</span>
        </div>
        <div class="dicom-presets-grid">
          <button class="dicom-preset-btn" data-preset-name="soft_tissue">
            <span>🫀 Tissus mous</span>
            <span class="dicom-preset-sub">40 / 400 HU</span>
          </button>
          <button class="dicom-preset-btn" data-preset-name="lung">
            <span>🫁 Poumons</span>
            <span class="dicom-preset-sub">-600 / 1500 HU</span>
          </button>
          <button class="dicom-preset-btn" data-preset-name="bone">
            <span>🦴 Os / Squelette</span>
            <span class="dicom-preset-sub">400 / 1800 HU</span>
          </button>
          <button class="dicom-preset-btn" data-preset-name="brain">
            <span>🧠 Cerveau / AVC</span>
            <span class="dicom-preset-sub">40 / 80 HU</span>
          </button>
          <button class="dicom-preset-btn" data-preset-name="mediastinum">
            <span>🫁 Médiastin</span>
            <span class="dicom-preset-sub">50 / 350 HU</span>
          </button>
          <button class="dicom-preset-btn" data-preset-name="default">
            <span>🔄 Réinitialiser</span>
            <span class="dicom-preset-sub">Défaut DICOM</span>
          </button>
        </div>
      </div>

      <!-- Section Réglage Manuel (VOI LUT) -->
      <div class="dicom-section">
        <div class="dicom-section-title">
          <span>🎚️ Fenêtrage Manuel (VOI LUT)</span>
        </div>
        
        <div class="dicom-control-group">
          <div class="dicom-slider-header">
            <span>Niveau (WC / Center) :</span>
            <span id="dicomValWc" style="font-weight: 700; color: #10b981; font-family: monospace;">${this.dicomWc} HU</span>
          </div>
          <input type="range" class="dicom-slider" id="dicomSliderWc" min="-1000" max="2000" step="5" value="${this.dicomWc}">
        </div>

        <div class="dicom-control-group">
          <div class="dicom-slider-header">
            <span>Largeur (WW / Width) :</span>
            <span id="dicomValWw" style="font-weight: 700; color: #10b981; font-family: monospace;">${this.dicomWw} HU</span>
          </div>
          <input type="range" class="dicom-slider" id="dicomSliderWw" min="1" max="4000" step="5" value="${this.dicomWw}">
        </div>

        <div class="dicom-hu-display" id="dicomHuRange">
          Plage : [${minHu} HU → ${maxHu} HU]
        </div>
      </div>
    `;

    // Presets buttons event listeners
    container.querySelectorAll('[data-preset-name]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const pName = btn.getAttribute('data-preset-name');
        if (pName) {
          this.applyDicomPreset(pName);
        }
      });
    });

    // Slider inputs
    const sliderWc = container.querySelector('#dicomSliderWc') as HTMLInputElement;
    const sliderWw = container.querySelector('#dicomSliderWw') as HTMLInputElement;

    const handleSlider = () => {
      const wc = parseInt(sliderWc.value, 10);
      const ww = parseInt(sliderWw.value, 10);
      this.setDicomWindow(wc, ww);
    };

    sliderWc.addEventListener('input', handleSlider);
    sliderWw.addEventListener('input', handleSlider);

    this.sidebarContent.appendChild(container);
  }

  private getPageThumbDimensions(pNum: number): { width: number; height: number; itemHeight: number } {
    const pageMeta = this.currentDoc?.pages[pNum - 1];
    const baseAspect = (pageMeta && pageMeta.height > 0) ? (pageMeta.width / pageMeta.height) : (595 / 842);
    const rot = this.getPageRotation(pNum);
    const isSideways = Math.abs(rot % 180) === 90;
    const aspect = isSideways ? (1 / baseAspect) : baseAspect;

    let width = 140;
    let height = 180;
    if (aspect >= 1.0) {
      // Landscape (e.g. 16:9, 4:3, or A4 landscape)
      width = 140;
      height = Math.max(40, Math.round(140 / aspect));
    } else {
      // Portrait (e.g. A4 portrait)
      height = 180;
      width = Math.max(40, Math.min(140, Math.round(180 * aspect)));
    }
    return { width, height, itemHeight: height + 28 };
  }

  private createThumbnailItem(pNum: number, virtualized: boolean = false, topOffset: number = 0): HTMLElement {
    const item = document.createElement('div');
    item.className = `thumb-item ${virtualized ? 'virtualized' : ''} ${pNum === this.activePage ? 'active' : ''}`;
    item.id = `thumb-${pNum}`;
    item.setAttribute('data-page', pNum.toString());
    if (virtualized) {
      item.style.top = `${topOffset}px`;
    }
    item.onclick = () => this.goToPage(pNum);

    const dims = this.getPageThumbDimensions(pNum);
    const preview = document.createElement('div');
    preview.className = 'thumb-preview';
    preview.style.width = `${dims.width}px`;
    preview.style.height = `${dims.height}px`;
    const rot = this.getPageRotation(pNum);
    const isSideways = Math.abs(rot % 180) === 90;
    preview.classList.toggle('rotated-sideways', isSideways);
    preview.style.setProperty('--thumb-rot', `${rot}deg`);

    const img = document.createElement('img');
    img.loading = 'lazy';
    img.style.transform = `translate(-50%, -50%) rotate(${rot}deg)`;

    // Quick rotate button on hover
    const rotateBtn = document.createElement('button');
    rotateBtn.className = 'thumb-rotate-btn';
    rotateBtn.title = `Pivoter la page ${pNum} de 90°`;
    rotateBtn.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/></svg>`;
    rotateBtn.onclick = (e) => {
      e.stopPropagation();
      this.rotate(90, pNum);
    };
    item.appendChild(rotateBtn);
    
    // If cached, display instantly; otherwise observe for lazy load
    const cached = this.cachedThumbBlobs.get(pNum);
    if (cached) {
      img.src = cached;
    } else if (pNum <= 3) {
      img.src = `/api/documents/${this.currentDoc!.id}/pages/${pNum}/thumbnail`;
    }

    preview.appendChild(img);

    const label = document.createElement('div');
    label.className = 'thumb-label';
    label.textContent = `Page ${pNum}`;

    item.appendChild(preview);
    item.appendChild(label);

    if (!img.src && this.thumbIntersectionObserver) {
      this.thumbIntersectionObserver.observe(item);
    }

    return item;
  }

  private updateVirtualizedThumbnails(container: HTMLElement, offsets: number[]) {
    if (!this.currentDoc || offsets.length === 0) return;
    const scrollTop = this.sidebarContent.scrollTop;
    const viewportHeight = this.sidebarContent.clientHeight || 800;
    const totalPages = this.currentDoc.page_count;

    let startIndex = 0;
    while (startIndex < totalPages - 1 && offsets[startIndex + 1] < scrollTop) {
      startIndex++;
    }
    startIndex = Math.max(0, startIndex - 1);

    let endIndex = startIndex;
    const viewBottom = scrollTop + viewportHeight;
    while (endIndex < totalPages - 1 && offsets[endIndex] < viewBottom) {
      endIndex++;
    }
    endIndex = Math.min(totalPages - 1, endIndex + 1);

    const pagesToLoad: number[] = [];
    for (let i = startIndex; i <= endIndex; i++) {
      pagesToLoad.push(i + 1);
    }
    if (this.renderWorker && pagesToLoad.length > 0) {
      this.renderWorker.postMessage({
        type: 'PRELOAD_THUMBNAILS',
        docId: this.currentDoc.id,
        pages: pagesToLoad,
      });
    }

    container.innerHTML = '';
    for (let i = startIndex; i <= endIndex; i++) {
      const pNum = i + 1;
      const el = this.createThumbnailItem(pNum, true, offsets[i]);
      container.appendChild(el);
    }
  }

  public searchInDocument(query: string) {
    this.searchMatches = [];
    this.currentSearchIndex = -1;

    if (!query.trim()) {
      this.clearSearch();
      return;
    }

    this.textRenderers.forEach((renderer, pageNum) => {
      const pageMatches = renderer.highlightSearch(query);
      for (const el of pageMatches) {
        this.searchMatches.push({ pageNumber: pageNum, element: el });
      }
    });

    const countEl = this.$('searchCount');
    if (countEl) {
      countEl.textContent = this.searchMatches.length > 0
        ? `0 / ${this.searchMatches.length}`
        : `0 trouvé`;
    }

    if (this.searchMatches.length > 0) {
      this.navigateSearch(1);
    }
  }

  private clearSearch() {
    this.textRenderers.forEach((renderer) => renderer.clearHighlights());
    this.searchMatches = [];
    this.currentSearchIndex = -1;
    const countEl = this.$('searchCount');
    if (countEl) countEl.textContent = '0/0';
  }

  private navigateSearch(direction: number) {
    if (this.searchMatches.length === 0) return;

    // Remove active highlight on current
    if (this.currentSearchIndex >= 0 && this.currentSearchIndex < this.searchMatches.length) {
      this.searchMatches[this.currentSearchIndex].element.classList.remove('highlight-active');
    }

    this.currentSearchIndex += direction;
    if (this.currentSearchIndex >= this.searchMatches.length) {
      this.currentSearchIndex = 0; // Loop back
    } else if (this.currentSearchIndex < 0) {
      this.currentSearchIndex = this.searchMatches.length - 1; // Loop to end
    }

    const match = this.searchMatches[this.currentSearchIndex];
    match.element.classList.add('highlight-active');

    // Scroll element into view
    match.element.scrollIntoView({ behavior: 'smooth', block: 'center' });

    // Update page number if different
    if (this.activePage !== match.pageNumber) {
      this.activePage = match.pageNumber;
      this.pageNumberInput.value = this.activePage.toString();
    }

    const countEl = this.$('searchCount');
    if (countEl) {
      countEl.textContent = `${this.currentSearchIndex + 1} / ${this.searchMatches.length}`;
    }
  }

  // --- PII / RGPD Assistant Methods ---

  public async runPiiScan() {
    if (!this.currentDoc) {
      alert('Veuillez ouvrir un document avant de lancer le scan RGPD.');
      return;
    }

    const piiModal = this.$('piiModal');
    if (piiModal) {
      piiModal.style.display = 'flex';
    }

    const loadingEl = this.$('piiScanLoading');
    const emptyEl = this.$('piiScanEmpty');
    const resultsEl = this.$('piiScanResults');
    const applyBtn = this.$('btnApplyPiiRedactions') as HTMLButtonElement;
    const countSummary = this.$('piiSummaryCount');

    if (loadingEl) loadingEl.style.display = 'block';
    if (emptyEl) emptyEl.style.display = 'none';
    if (resultsEl) {
      resultsEl.style.display = 'none';
      resultsEl.innerHTML = '';
    }
    if (applyBtn) applyBtn.disabled = true;
    if (countSummary) countSummary.textContent = '';

    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/pii-scan`);
      if (!resp.ok) {
        alert('Échec de l\'analyse PII');
        return;
      }

      const data = await resp.json();
      this.currentPiiItems = data.items || [];

      if (loadingEl) loadingEl.style.display = 'none';

      if (this.currentPiiItems.length === 0) {
        if (emptyEl) emptyEl.style.display = 'block';
        return;
      }

      if (resultsEl) {
        resultsEl.style.display = 'block';
        resultsEl.innerHTML = '';

        for (let i = 0; i < this.currentPiiItems.length; i++) {
          const item = this.currentPiiItems[i];
          const row = document.createElement('div');
          row.className = 'pii-item-row';

          const badgeClass = `pii-badge-${item.category}`;
          const categoryName = {
            iban: 'IBAN SEPA',
            credit_card: 'Carte Bancaire',
            social_security: 'Sécurité Sociale (NIR)',
            email: 'E-mail',
            phone: 'Téléphone',
          }[item.category as string] || item.category;

          row.innerHTML = `
            <input type="checkbox" id="pii-chk-${i}" checked style="cursor: pointer;">
            <span class="pii-badge ${badgeClass}">${categoryName}</span>
            <div style="flex: 1; font-family: monospace;">
              <strong>${item.masked_preview}</strong>
              <span style="color: var(--text-muted); font-size: 11px; margin-left: 8px;">(P.${item.page_number})</span>
            </div>
          `;

          row.onclick = (e) => {
            if ((e.target as HTMLElement).tagName !== 'INPUT') {
              const chk = row.querySelector('input') as HTMLInputElement;
              if (chk) chk.checked = !chk.checked;
            }
            this.updatePiiApplyButtonState();
          };

          resultsEl.appendChild(row);
        }
      }

      if (countSummary) {
        countSummary.textContent = `${this.currentPiiItems.length} élément(s) sensible(s) détecté(s)`;
      }
      if (applyBtn) applyBtn.disabled = false;
    } catch (e) {
      console.error('PII scan error', e);
      if (loadingEl) loadingEl.style.display = 'none';
      alert(`Erreur d'analyse: ${e}`);
    }
  }

  private updatePiiApplyButtonState() {
    const applyBtn = this.$('btnApplyPiiRedactions') as HTMLButtonElement;
    const checkboxes = this.$$('#piiScanResults input[type="checkbox"]:checked');
    if (applyBtn) {
      applyBtn.disabled = checkboxes.length === 0;
      applyBtn.textContent = `Biffer la sélection (${checkboxes.length}) en 1-clic`;
    }
  }

  private async applySelectedPiiRedactions() {
    if (!this.currentDoc) return;

    const checkboxes = this.$$('#piiScanResults input[type="checkbox"]');
    const selectedItems: any[] = [];

    checkboxes.forEach((cb, idx) => {
      if ((cb as HTMLInputElement).checked && this.currentPiiItems[idx]) {
        const it = this.currentPiiItems[idx];
        selectedItems.push({
          page_number: it.page_number,
          x: it.x,
          y: it.y,
          width: it.width,
          height: it.height,
          reason: 'RGPD / Donnée Confidentielle',
          overlay_text: 'DONNEE SENSIBLE RGPD',
        });
      }
    });

    if (selectedItems.length === 0) {
      alert('Veuillez sélectionner au moins un élément à biffer.');
      return;
    }

    const piiModal = this.$('piiModal');
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/redact`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          document_id: this.currentDoc.id,
          items: selectedItems,
        }),
      });

      if (!resp.ok) {
        alert('Échec de la biffure automatique.');
        return;
      }

      const newDocMeta = await resp.json();
      if (piiModal) piiModal.style.display = 'none';
      alert(`Biffure de conformité RGPD appliquée sur ${selectedItems.length} élément(s) ! Chargement du document sécurisé...`);
      this.loadDocumentById(newDocMeta.id);
    } catch (e) {
      alert(`Erreur lors de l'application de la biffure RGPD: ${e}`);
    }
  }


  private updateBurnInButton() {
    const hasRedactions = this.allAnnotations.some((a) => a.annotation_type === 'redact');
    const btn = this.$('btnOpenBurnIn');
    if (btn) {
      btn.style.display = hasRedactions ? 'flex' : 'none';
    }
  }

  private setupSelectionToolbar() {
    const toolbar = document.createElement('div');
    toolbar.className = 'selection-floating-toolbar';
    toolbar.style.display = 'none';
    toolbar.innerHTML = `
      <button id="selBtnHighlight">🖍️ Surligner</button>
      <button id="selBtnNote">📝 Note</button>
      <button id="selBtnRedact">⬛ Biffer</button>
    `;
    document.body.appendChild(toolbar);

    const hideToolbar = () => {
      toolbar.style.display = 'none';
    };

    document.addEventListener('selectionchange', () => {
      if (this.currentTool !== 'select') {
        hideToolbar();
        return;
      }
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed || !sel.toString().trim()) {
        hideToolbar();
        return;
      }
      if (sel.rangeCount === 0) {
        hideToolbar();
        return;
      }

      const range = sel.getRangeAt(0);
      const rect = range.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {
        hideToolbar();
        return;
      }

      if (!this.pagesContainer) {
        hideToolbar();
        return;
      }
      const containerRect = this.pagesContainer.getBoundingClientRect();
      if (
        rect.bottom < containerRect.top ||
        rect.top > containerRect.bottom ||
        rect.right < containerRect.left ||
        rect.left > containerRect.right
      ) {
        hideToolbar();
        return;
      }

      toolbar.style.display = 'flex';
      const tbWidth = 240;
      const left = Math.max(10, Math.min(window.innerWidth - tbWidth - 10, rect.left + rect.width / 2 - tbWidth / 2));
      const top = Math.max(10, rect.top - 46);
      toolbar.style.left = `${left}px`;
      toolbar.style.top = `${top}px`;
    });

    toolbar.querySelector('#selBtnHighlight')?.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.applySelectionAnnotation('highlight');
      hideToolbar();
    });

    toolbar.querySelector('#selBtnRedact')?.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.applySelectionAnnotation('redact');
      hideToolbar();
    });

    toolbar.querySelector('#selBtnNote')?.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.applySelectionNote();
      hideToolbar();
    });

    window.addEventListener('mousedown', (e) => {
      if (!toolbar.contains(e.target as Node)) {
        hideToolbar();
      }
    });
  }

  private applySelectionAnnotation(type: 'highlight' | 'redact') {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
    const range = sel.getRangeAt(0);

    this.annotationManagers.forEach((mgr) => {
      mgr.createAnnotationsFromSelectionRange(range, type);
    });

    sel.removeAllRanges();
  }

  private applySelectionNote() {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
    const range = sel.getRangeAt(0);
    const text = sel.toString().trim();
    const rects = range.getClientRects();

    if (rects.length > 0) {
      const lastRect = rects[rects.length - 1];
      for (const [pageNum, mgr] of this.annotationManagers.entries()) {
        const pageEl = this.$(`page-${pageNum}`);
        if (pageEl) {
          const pRect = pageEl.getBoundingClientRect();
          if (lastRect.bottom >= pRect.top && lastRect.top <= pRect.bottom) {
            const x = Math.max(10, lastRect.right - pRect.left + 5);
            const y = Math.max(10, lastRect.bottom - pRect.top + 5);
            mgr.openNoteCreatePopover(x, y, `« ${text.slice(0, 60)}${text.length > 60 ? '...' : ''} »\n`);
            break;
          }
        }
      }
    }

    sel.removeAllRanges();
  }

  private async loadAnnotations() {
    if (!this.currentDoc) return;
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/annotations`);
      if (resp.ok) {
        this.allAnnotations = await resp.json();
        this.annotationManagers.forEach((mgr) => mgr.setAnnotations(this.allAnnotations));
        this.updateBurnInButton();
        this.updateAnnotationsBadge();
      }
    } catch (e) {
      console.warn('Failed to load annotations', e);
    }
  }

  private async saveAnnotations() {
    if (!this.currentDoc) return;
    try {
      await fetch(`/api/documents/${this.currentDoc.id}/annotations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(this.allAnnotations),
      });
      this.updateBurnInButton();
      this.updateAnnotationsBadge();
      this.dispatchEvent('annotationsaved', { count: this.allAnnotations.length });
    } catch (e) {
      console.error('Failed to save annotations', e);
    }
  }

  private updateAnnotationsBadge() {
    const badge = this.$('annotationsBadge');
    if (badge) {
      if (this.allAnnotations.length > 0) {
        badge.style.display = 'flex';
        badge.textContent = this.allAnnotations.length.toString();
      } else {
        badge.style.display = 'none';
      }
    }
  }

  public async loadFormFields(): Promise<FormFieldsSummary | null> {
    if (!this.currentDoc) return null;
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/forms`);
      if (!resp.ok) return null;
      const summary: FormFieldsSummary = await resp.json();
      this.currentForms = summary;
      this.formRenderer.setFields(summary.fields);

      const btnSave = this.$('btnSaveForms');
      const formsTab = this.$('formsTab');
      const formsBadge = this.$('formsBadge');

      if (summary.has_forms) {
        if (btnSave) btnSave.style.display = 'inline-flex';
        if (formsTab) formsTab.style.display = 'flex';
        if (formsBadge) {
          formsBadge.style.display = 'flex';
          formsBadge.textContent = summary.fields_count.toString();
        }
        // Render onto currently rendered pages
        document.querySelectorAll('.page-container[data-rendered="true"]').forEach((card) => {
          const pNum = parseInt((card as HTMLElement).dataset.pageNumber || '1', 10);
          this.formRenderer.render(card as HTMLElement, pNum, this.currentZoom);
        });
      } else {
        if (btnSave) btnSave.style.display = 'none';
        if (formsTab) formsTab.style.display = 'none';
        if (formsBadge) formsBadge.style.display = 'none';
      }

      this.dispatchEvent('formloaded', summary);
      return summary;
    } catch (e) {
      console.error('Failed to load forms:', e);
      return null;
    }
  }

  public getFormFields(): FormField[] {
    return this.formRenderer.getFields();
  }

  public getFormValues(): Record<string, any> {
    return this.formRenderer.getAllValues();
  }

  public setFormFieldValue(name: string, value: any) {
    this.formRenderer.setValue(name, value);
    this.dispatchEvent('formfieldchange', { field: { name }, value });
  }

  public async saveFormValues(saveAsNew = false): Promise<FormFillResponse | null> {
    if (!this.currentDoc) return null;
    const values = this.formRenderer.getAllValues();
    try {
      const resp = await fetch(`/api/documents/${this.currentDoc.id}/forms/fill`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ values, save_as_new: saveAsNew }),
      });
      if (!resp.ok) {
        const err = await resp.json();
        alert(`Erreur d'enregistrement du formulaire: ${err.error || resp.statusText}`);
        return null;
      }
      const result: FormFillResponse = await resp.json();
      this.dispatchEvent('formsaved', result);

      // Refresh page images to reflect /NeedAppearances rasterization
      for (const url of this.cachedPageBlobs.values()) URL.revokeObjectURL(url);
      this.cachedPageBlobs.clear();
      this.renderWorker?.postMessage({ type: 'CLEAR_CACHE' });

      document.querySelectorAll('.page-container[data-rendered="true"]').forEach((card) => {
        const pNum = parseInt((card as HTMLElement).dataset.pageNumber || '1', 10);
        const img = card.querySelector('.page-image') as HTMLImageElement;
        if (img) {
          img.src = `/api/documents/${result.document_id}/pages/${pNum}/render?dpi=120&t=${Date.now()}`;
        }
      });

      alert(`✅ Formulaire enregistré (${result.updated_fields_count} champs mis à jour).`);
      return result;
    } catch (e) {
      console.error('Error saving form values:', e);
      alert('Erreur réseau lors de la sauvegarde du formulaire.');
      return null;
    }
  }

  // =========================================================================
  // Multimedia & Video Player
  // =========================================================================

  private renderVideoPlayer(meta: DocumentMetadata) {
    this.pagesContainer.innerHTML = '';
    this.pagesContainer.style.display = 'flex';
    this.pagesContainer.style.alignItems = 'center';
    this.pagesContainer.style.justifyContent = 'center';
    this.pagesContainer.style.width = '100%';
    this.pagesContainer.style.height = '100%';
    this.pagesContainer.style.backgroundColor = '#000000';

    const wrapper = document.createElement('div');
    wrapper.className = 'video-viewer-wrapper';

    wrapper.innerHTML = `
      <div class="video-container" id="videoContainer">
        <video
          id="oxidVideoPlayer"
          class="oxid-video-element"
          src="/api/documents/${meta.id}/video"
          poster="/api/documents/${meta.id}/pages/1/render?dpi=120"
          preload="metadata"
          playsinline
        ></video>
        <div class="video-controls-overlay" id="videoControlsOverlay">
          <button class="video-big-play-btn" id="videoBigPlayBtn" title="Lecture (Espace)">
            <svg viewBox="0 0 24 24" width="36" height="36"><polygon points="6 3 20 12 6 21 6 3" fill="white"/></svg>
          </button>
          <div class="video-control-bar" id="videoControlBar">
            <button class="video-btn" id="videoPlayPauseBtn" title="Lecture / Pause (Espace)">
              <svg id="vIconPlay" viewBox="0 0 24 24" width="20" height="20"><polygon points="5 3 19 12 5 21 5 3" fill="currentColor"/></svg>
              <svg id="vIconPause" viewBox="0 0 24 24" width="20" height="20" style="display:none;"><rect x="6" y="4" width="4" height="16" fill="currentColor"/><rect x="14" y="4" width="4" height="16" fill="currentColor"/></svg>
            </button>
            <span class="video-time" id="videoTimeDisplay">00:00 / 00:00</span>
            <div class="video-timeline-container" id="videoTimeline">
              <div class="video-timeline-bg">
                <div class="video-timeline-buffered" id="videoBufferedBar"></div>
                <div class="video-timeline-progress" id="videoProgressBar"></div>
              </div>
              <input type="range" class="video-seek-slider" id="videoSeekSlider" min="0" max="100" value="0" step="0.1">
            </div>
            <button class="video-btn" id="videoMuteBtn" title="Muet (M)">
              <svg id="vIconVol" viewBox="0 0 24 24" width="20" height="20"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" fill="currentColor"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14" stroke="currentColor" stroke-width="2" fill="none"/></svg>
              <svg id="vIconMuted" viewBox="0 0 24 24" width="20" height="20" style="display:none;"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" fill="currentColor"/><line x1="23" y1="9" x2="17" y2="15" stroke="currentColor" stroke-width="2"/><line x1="17" y1="9" x2="23" y2="15" stroke="currentColor" stroke-width="2"/></svg>
            </button>
            <input type="range" class="video-volume-slider" id="videoVolumeSlider" min="0" max="1" step="0.05" value="1" title="Volume">
            <select class="video-speed-select" id="videoSpeedSelect" title="Vitesse de lecture">
              <option value="0.5">0.5x</option>
              <option value="0.75">0.75x</option>
              <option value="1" selected>1.0x</option>
              <option value="1.25">1.25x</option>
              <option value="1.5">1.5x</option>
              <option value="2">2.0x</option>
            </select>
            <button class="video-btn" id="videoLoopBtn" title="Lecture en boucle">
              <svg viewBox="0 0 24 24" width="18" height="18"><path d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z" fill="currentColor"/></svg>
            </button>
            <button class="video-btn" id="videoPipBtn" title="Incrustation (Picture-in-Picture)">
              <svg viewBox="0 0 24 24" width="18" height="18"><rect x="2" y="4" width="20" height="16" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><rect x="12" y="11" width="8" height="7" rx="1" fill="currentColor"/></svg>
            </button>
            <button class="video-btn" id="videoFullscreenBtn" title="Plein écran (F)">
              <svg viewBox="0 0 24 24" width="18" height="18"><path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z" fill="currentColor"/></svg>
            </button>
          </div>
        </div>
      </div>
    `;

    this.pagesContainer.appendChild(wrapper);

    const video = wrapper.querySelector('#oxidVideoPlayer') as HTMLVideoElement;
    const bigPlayBtn = wrapper.querySelector('#videoBigPlayBtn') as HTMLButtonElement;
    const playPauseBtn = wrapper.querySelector('#videoPlayPauseBtn') as HTMLButtonElement;
    const iconPlay = wrapper.querySelector('#vIconPlay') as SVGElement;
    const iconPause = wrapper.querySelector('#vIconPause') as SVGElement;
    const timeDisplay = wrapper.querySelector('#videoTimeDisplay') as HTMLElement;
    const progressBar = wrapper.querySelector('#videoProgressBar') as HTMLElement;
    const bufferedBar = wrapper.querySelector('#videoBufferedBar') as HTMLElement;
    const seekSlider = wrapper.querySelector('#videoSeekSlider') as HTMLInputElement;
    const muteBtn = wrapper.querySelector('#videoMuteBtn') as HTMLButtonElement;
    const iconVol = wrapper.querySelector('#vIconVol') as SVGElement;
    const iconMuted = wrapper.querySelector('#vIconMuted') as SVGElement;
    const volumeSlider = wrapper.querySelector('#videoVolumeSlider') as HTMLInputElement;
    const speedSelect = wrapper.querySelector('#videoSpeedSelect') as HTMLSelectElement;
    const loopBtn = wrapper.querySelector('#videoLoopBtn') as HTMLButtonElement;
    const pipBtn = wrapper.querySelector('#videoPipBtn') as HTMLButtonElement;
    const fullscreenBtn = wrapper.querySelector('#videoFullscreenBtn') as HTMLButtonElement;
    const videoContainer = wrapper.querySelector('#videoContainer') as HTMLElement;

    const formatTime = (secs: number) => {
      const m = Math.floor(secs / 60);
      const s = Math.floor(secs % 60);
      return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    };

    const updatePlayState = () => {
      if (video.paused) {
        iconPlay.style.display = 'block';
        iconPause.style.display = 'none';
        bigPlayBtn.style.display = 'flex';
      } else {
        iconPlay.style.display = 'none';
        iconPause.style.display = 'block';
        bigPlayBtn.style.display = 'none';
      }
    };

    const togglePlay = () => {
      if (video.paused) {
        video.play();
      } else {
        video.pause();
      }
    };

    playPauseBtn?.addEventListener('click', togglePlay);
    bigPlayBtn?.addEventListener('click', togglePlay);
    video?.addEventListener('click', togglePlay);
    video?.addEventListener('play', updatePlayState);
    video?.addEventListener('pause', updatePlayState);

    video?.addEventListener('timeupdate', () => {
      if (!isNaN(video.duration) && video.duration > 0) {
        const pct = (video.currentTime / video.duration) * 100;
        progressBar.style.width = `${pct}%`;
        seekSlider.value = pct.toString();
        timeDisplay.textContent = `${formatTime(video.currentTime)} / ${formatTime(video.duration)}`;
      } else {
        timeDisplay.textContent = `${formatTime(video.currentTime)} / 00:00`;
      }
    });

    video?.addEventListener('progress', () => {
      if (video.buffered.length > 0 && !isNaN(video.duration) && video.duration > 0) {
        const bufferedEnd = video.buffered.end(video.buffered.length - 1);
        bufferedBar.style.width = `${(bufferedEnd / video.duration) * 100}%`;
      }
    });

    seekSlider?.addEventListener('input', () => {
      if (!isNaN(video.duration) && video.duration > 0) {
        const targetTime = (parseFloat(seekSlider.value) / 100) * video.duration;
        video.currentTime = targetTime;
      }
    });

    const updateVolumeUI = () => {
      if (video.muted || video.volume === 0) {
        iconVol.style.display = 'none';
        iconMuted.style.display = 'block';
      } else {
        iconVol.style.display = 'block';
        iconMuted.style.display = 'none';
      }
      volumeSlider.value = video.muted ? '0' : video.volume.toString();
    };

    muteBtn?.addEventListener('click', () => {
      video.muted = !video.muted;
      updateVolumeUI();
    });

    volumeSlider?.addEventListener('input', () => {
      video.volume = parseFloat(volumeSlider.value);
      video.muted = video.volume === 0;
      updateVolumeUI();
    });

    speedSelect?.addEventListener('change', () => {
      video.playbackRate = parseFloat(speedSelect.value);
    });

    loopBtn?.addEventListener('click', () => {
      video.loop = !video.loop;
      loopBtn.style.color = video.loop ? '#10b981' : '#e2e8f0';
    });

    pipBtn?.addEventListener('click', async () => {
      try {
        if (document.pictureInPictureElement) {
          await document.exitPictureInPicture();
        } else if (document.pictureInPictureEnabled) {
          await video.requestPictureInPicture();
        }
      } catch (e) {
        console.warn('PiP not available:', e);
      }
    });

    const toggleFullscreen = () => {
      if (!document.fullscreenElement) {
        videoContainer.requestFullscreen().catch((err) => console.warn(err));
      } else {
        document.exitFullscreen().catch((err) => console.warn(err));
      }
    };

    fullscreenBtn?.addEventListener('click', toggleFullscreen);
    video?.addEventListener('dblclick', toggleFullscreen);

    const updateFullscreenBtnUI = () => {
      const isFs = !!document.fullscreenElement;
      fullscreenBtn.title = isFs ? 'Quitter le plein écran (F)' : 'Plein écran (F)';
      fullscreenBtn.innerHTML = isFs
        ? `<svg viewBox="0 0 24 24" width="18" height="18"><path d="M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-14v3h3v2h-5V5h2z" fill="currentColor"/></svg>`
        : `<svg viewBox="0 0 24 24" width="18" height="18"><path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z" fill="currentColor"/></svg>`;
    };

    document.addEventListener('fullscreenchange', updateFullscreenBtnUI);

    // Auto-hide controls & cursor during playback
    let hideControlsTimeout: any = null;
    const controlsOverlay = wrapper.querySelector('#videoControlsOverlay') as HTMLElement;

    const resetControlsTimeout = () => {
      if (controlsOverlay) controlsOverlay.style.opacity = '1';
      videoContainer.style.cursor = 'default';
      clearTimeout(hideControlsTimeout);
      if (!video.paused) {
        hideControlsTimeout = setTimeout(() => {
          if (!video.paused && controlsOverlay) {
            controlsOverlay.style.opacity = '0';
            videoContainer.style.cursor = 'none';
          }
        }, 2500);
      }
    };

    videoContainer.addEventListener('mousemove', resetControlsTimeout);
    videoContainer.addEventListener('click', resetControlsTimeout);
    video.addEventListener('play', resetControlsTimeout);
    video.addEventListener('pause', () => {
      clearTimeout(hideControlsTimeout);
      if (controlsOverlay) controlsOverlay.style.opacity = '1';
      videoContainer.style.cursor = 'default';
    });

    // Keyboard shortcuts
    const handleVideoKey = (e: KeyboardEvent) => {
      if (document.activeElement && ['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement.tagName)) return;
      if (e.key === ' ' || e.code === 'Space') {
        e.preventDefault();
        togglePlay();
      } else if (e.key === 'f' || e.key === 'F') {
        e.preventDefault();
        toggleFullscreen();
      } else if (e.key === 'm' || e.key === 'M') {
        e.preventDefault();
        video.muted = !video.muted;
        updateVolumeUI();
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        video.currentTime = Math.min(video.duration || 0, video.currentTime + 5);
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        video.currentTime = Math.max(0, video.currentTime - 5);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        video.volume = Math.min(1, video.volume + 0.1);
        video.muted = false;
        updateVolumeUI();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        video.volume = Math.max(0, video.volume - 0.1);
        updateVolumeUI();
      }
    };
    window.addEventListener('keydown', handleVideoKey);
  }
}


// Auto bootstrap only for standalone index.html (when <oxid-viewer> Web Component is not used)
if (typeof window !== 'undefined') {
  const initStandalone = () => {
    // Only auto-instantiate if there is NO <oxid-viewer> Web Component in DOM
    if (!document.querySelector('oxid-viewer')) {
      if (!(window as any).oxidViewer) {
        (window as any).oxidViewer = new OxidViewer();
      }
    }
  };

  if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', initStandalone);
  } else {
    initStandalone();
  }
}
