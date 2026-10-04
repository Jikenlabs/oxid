import { OxidViewer } from './viewer.ts';
import { VIEWER_TEMPLATE } from './template.ts';
import styleText from './style.css?inline';

export class OxidViewerElement extends HTMLElement {
  public viewer: OxidViewer | null = null;
  private container: HTMLElement | null = null;

  private pendingPermissions: any = null;
  private pendingWatermark: string | null = null;

  static get observedAttributes() {
    return [
      'doc-id', 'src', 'connector', 'token', 'theme', 'watermark', 'permissions',
      'read-only',
      'disable-download', 'disable-print', 'disable-redaction', 'disable-annotation',
      'disable-sign', 'disable-build', 'disable-upload',
      'disable-rotate', 'disable-view-mode', 'disable-scroll-mode', 'disable-zen-mode', 'disable-search'
    ];
  }

  connectedCallback() {
    if (this.viewer) return;

    // Inject styles and template into element
    this.innerHTML = `
      <style>
        oxid-viewer, .oxid-viewer-root {
          display: flex;
          flex-direction: column;
          width: 100%;
          height: 100%;
          overflow: hidden;
          position: relative;
          background-color: var(--bg-primary, #0f172a);
          color: var(--text-main, #f8fafc);
        }
        ${styleText}
      </style>
      ${VIEWER_TEMPLATE}
    `;

    this.container = this.querySelector('.oxid-viewer-root');

    // Instantiate OxidViewer attached to this container
    this.viewer = new OxidViewer(this.container as HTMLElement);
    (window as any).oxidViewer = this.viewer;

    // Forward custom events
    this.setupEventForwarding();

    // Check initial attributes
    const docId = this.getAttribute('doc-id');
    const src = this.getAttribute('src');
    const connector = this.getAttribute('connector');
    const token = this.getAttribute('token');

    const watermark = this.pendingWatermark ?? this.getAttribute('watermark');
    if (watermark) {
      this.viewer.setWatermark(watermark);
    }
    this.updatePermissionsFromAttributes();
    if (this.pendingPermissions) {
      this.viewer.setPermissions(this.pendingPermissions);
    }

    if (src) {
      this.viewer.loadFromUrl(src);
    } else if (connector && docId) {
      this.viewer.loadRemote(connector, docId, token || undefined);
    } else if (docId) {
      this.viewer.loadDocumentById(docId);
    }
  }

  attributeChangedCallback(name: string, oldValue: string, newValue: string) {
    if (oldValue === newValue || !this.viewer) return;

    if (name === 'watermark') {
      this.viewer.setWatermark(newValue || '');
    } else if (name === 'permissions' || name.startsWith('disable-') || name === 'read-only') {
      this.updatePermissionsFromAttributes();
    } else if (name === 'doc-id' && newValue) {
      this.viewer.loadDocumentById(newValue);
    } else if (name === 'src' && newValue) {
      this.viewer.loadFromUrl(newValue);
    }
  }

  private setupEventForwarding() {
    // Re-dispatch viewer events on the custom element itself
    const eventNames = [
      'documentloaded',
      'pagechanged',
      'toolchanged',
      'annotationsaved',
      'burninapplied',
      'uploadstart',
      'uploaderror',
      'filedropped',
      'formloaded',
      'formfieldchange',
      'formsaved',
      'permissionschanged',
      'viewmodechanged',
      'scrollmodechanged',
      'pagerotated',
      'zenmodechanged',
    ];
    eventNames.forEach((name) => {
      this.container?.addEventListener(name, (e: any) => {
        this.dispatchEvent(new CustomEvent(name, {
          bubbles: true,
          composed: true,
          detail: e.detail,
        }));
      });
    });
  }

  private updatePermissionsFromAttributes() {
    if (!this.viewer) return;
    const permsAttr = this.getAttribute('permissions');
    let permsObj: any = {};
    if (permsAttr) {
      try {
        permsObj = JSON.parse(permsAttr);
      } catch (e) {
        console.warn('Invalid JSON in permissions attribute', permsAttr);
      }
    }
    if (this.hasAttribute('disable-download')) permsObj.canDownload = false;
    if (this.hasAttribute('disable-print')) permsObj.canPrint = false;
    if (this.hasAttribute('disable-redaction')) permsObj.canRedact = false;
    if (this.hasAttribute('disable-annotation')) permsObj.canAnnotate = false;
    if (this.hasAttribute('disable-sign')) permsObj.canSign = false;
    if (this.hasAttribute('disable-build')) permsObj.canBuild = false;
    if (this.hasAttribute('disable-upload')) permsObj.canUpload = false;
    if (this.hasAttribute('disable-rotate')) permsObj.canRotate = false;
    if (this.hasAttribute('disable-view-mode')) permsObj.canChangeViewMode = false;
    if (this.hasAttribute('disable-scroll-mode')) permsObj.canChangeScrollMode = false;
    if (this.hasAttribute('disable-zen-mode')) permsObj.canZenMode = false;
    if (this.hasAttribute('disable-search')) permsObj.canSearch = false;
    if (this.hasAttribute('read-only')) permsObj.readOnly = true;

    this.viewer.setPermissions(permsObj);
  }

  public setWatermark(text: string) {
    this.pendingWatermark = text;
    this.viewer?.setWatermark(text);
  }

  public setPermissions(perms: any) {
    this.pendingPermissions = perms;
    this.viewer?.setPermissions(perms);
  }

  public print() {
    this.viewer?.print();
  }

  // --- Public API ---

  public uploadDocument(file: File) {
    this.viewer?.uploadDocument(file);
  }

  public loadDocumentById(id: string) {
    this.viewer?.loadDocumentById(id);
  }

  public loadFromUrl(url: string) {
    this.viewer?.loadFromUrl(url);
  }

  public loadRemote(connector: string, resourceId: string, token?: string) {
    this.viewer?.loadRemote(connector, resourceId, token);
  }

  public goToPage(page: number) {
    this.viewer?.goToPage(page);
  }

  public setZoom(zoom: number) {
    this.viewer?.setZoom(zoom);
  }

  public fitWidth() {
    this.viewer?.fitWidth();
  }

  public fitPage() {
    this.viewer?.fitPage();
  }

  public rotate(deg: number, scope?: 'current' | 'all' | number) {
    this.viewer?.rotate(deg, scope);
  }

  public setTool(tool: string) {
    this.viewer?.setTool(tool);
  }

  public runPiiScan() {
    this.viewer?.runPiiScan();
  }

  public search(query: string) {
    this.viewer?.searchInDocument(query);
  }

  public download() {
    this.viewer?.download();
  }

  public openSignatureDialog() {
    this.viewer?.openSignatureDialog();
  }

  public openDocumentBuilder() {
    this.viewer?.openDocumentBuilder();
  }

  public openComparisonDialog() {
    this.viewer?.openComparisonDialog();
  }

  public openRedactionDialog() {
    this.viewer?.openRedactionDialog();
  }

  public toggleSidebar(tab?: string) {
    this.viewer?.toggleSidebar(tab);
  }

  public getMetadata() {
    return this.viewer?.getMetadata() ?? null;
  }

  public getAnnotations() {
    return this.viewer?.getAnnotations() ?? [];
  }

  public getFormFields() {
    return this.viewer?.getFormFields() ?? [];
  }

  public getFormValues() {
    return this.viewer?.getFormValues() ?? {};
  }

  public setFormFieldValue(name: string, value: any) {
    this.viewer?.setFormFieldValue(name, value);
  }

  public setViewMode(mode: 'single' | 'double' | 'grid') {
    this.viewer?.setViewMode(mode);
  }

  public setScrollMode(mode: 'continuous' | 'page') {
    this.viewer?.setScrollMode(mode);
  }

  public nextSpread() {
    this.viewer?.nextSpread();
  }

  public prevSpread() {
    this.viewer?.prevSpread();
  }

  public toggleFullscreen() {
    this.viewer?.toggleFullscreen();
  }

  public isFullscreen(): boolean {
    return this.viewer?.isFullscreen() ?? false;
  }

  public get viewMode(): 'single' | 'double' | 'grid' {
    return this.viewer?.viewMode ?? 'single';
  }

  public set viewMode(mode: 'single' | 'double' | 'grid') {
    this.setViewMode(mode);
  }

  public get scrollMode(): 'continuous' | 'page' {
    return this.viewer?.scrollMode ?? 'continuous';
  }

  public set scrollMode(mode: 'continuous' | 'page') {
    this.setScrollMode(mode);
  }

  public toggleZenMode() {
    this.viewer?.toggleZenMode();
  }

  public setZenMode(enabled: boolean) {
    this.viewer?.setZenMode(enabled);
  }

  public get zenMode(): boolean {
    return this.viewer?.zenMode ?? false;
  }

  public set zenMode(enabled: boolean) {
    this.setZenMode(enabled);
  }

  public saveFormValues(saveAsNew = false) {
    return this.viewer?.saveFormValues(saveAsNew);
  }

  public loadFormFields() {
    return this.viewer?.loadFormFields();
  }

  // CAD / DAO Plans
  public getCadLayers() {
    return this.viewer?.getCadLayers() ?? [];
  }

  public toggleCadLayer(layerName: string, visible?: boolean) {
    this.viewer?.toggleCadLayer(layerName, visible);
  }

  public setCadLayers(layers: string[]) {
    this.viewer?.setCadLayers(layers);
  }

  // DICOM Medical Imaging
  public getDicomMetadata() {
    return this.viewer?.getDicomMetadata() ?? null;
  }

  public applyDicomPreset(presetName: string) {
    this.viewer?.applyDicomPreset(presetName);
  }

  public setDicomWindow(center: number, width: number) {
    this.viewer?.setDicomWindow(center, width);
  }

  public toggleDicomCine() {
    this.viewer?.toggleDicomCine();
  }

  public stopDicomCine() {
    this.viewer?.stopDicomCine();
  }
}

// Register Custom Element
if (typeof window !== 'undefined' && !customElements.get('oxid-viewer')) {
  customElements.define('oxid-viewer', OxidViewerElement);
}
