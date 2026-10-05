export { OxidViewerElement } from './oxid-element.ts';
export { OxidViewer } from './viewer.ts';
export type { DocumentMetadata } from './viewer.ts';
export type { ClientAnnotation } from './annotation-layer.ts';

// Enregistrement automatique de l'élément personnalisé lors de l'exécution dans le navigateur
import { OxidViewerElement } from './oxid-element.ts';

if (typeof window !== 'undefined' && !customElements.get('oxid-viewer')) {
  customElements.define('oxid-viewer', OxidViewerElement);
}
