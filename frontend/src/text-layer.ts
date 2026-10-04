export interface TextSpan {
  text: string;
  x: f64;
  y: f64;
  width: f64;
  height: f64;
  font_size: f64;
}

type f64 = number;

export interface PageText {
  page_number: number;
  spans: TextSpan[];
}

export class TextLayerRenderer {
  private container: HTMLElement;
  private spans: TextSpan[] = [];
  private scale: number = 1.0;

  constructor(container: HTMLElement) {
    this.container = container;
  }

  public async loadText(docId: string, pageNumber: number, pageWidth: number, pageHeight: number, renderedWidth: number): Promise<void> {
    try {
      const resp = await fetch(`/api/documents/${docId}/pages/${pageNumber}/text`);
      if (!resp.ok) return;
      const data: PageText = await resp.json();
      this.spans = data.spans;

      // Scale factor between PDF points and actual rendered DOM width
      this.scale = renderedWidth / pageWidth;
      this.render();
    } catch (e) {
      console.warn('Text layer loading skipped', e);
    }
  }

  public updateScale(scaleRatio: number) {
    this.scale = scaleRatio;
    this.render();
  }

  private render() {
    this.container.innerHTML = '';
    const fragment = document.createDocumentFragment();

    const fontStack = '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif';
    const measureCanvas = typeof document !== 'undefined' ? document.createElement('canvas') : null;
    const measureCtx = measureCanvas?.getContext('2d');

    for (let i = 0; i < this.spans.length; i++) {
      const span = this.spans[i];
      const el = document.createElement('span');
      el.textContent = span.text;
      
      const targetX = span.x * this.scale;
      const targetY = span.y * this.scale;
      const targetW = span.width * this.scale;
      const targetH = span.height * this.scale;

      el.style.left = `${targetX}px`;
      el.style.top = `${targetY}px`;
      el.style.height = `${targetH}px`;
      el.style.lineHeight = `${targetH}px`;
      el.style.transformOrigin = '0% 0%';

      const fontSize = Math.max(8, targetH * 0.78);
      el.style.fontSize = `${fontSize}px`;
      el.style.fontFamily = fontStack;

      // Exact horizontal scaling without hardcoded width to ensure 100% pixel alignment without overflow
      if (measureCtx && targetW > 0) {
        measureCtx.font = `${fontSize}px ${fontStack}`;
        const measuredW = measureCtx.measureText(span.text).width;
        if (measuredW > 0) {
          const scaleX = targetW / measuredW;
          el.style.transform = `scaleX(${scaleX.toFixed(4)})`;
        }
      }

      fragment.appendChild(el);
    }
    this.container.appendChild(fragment);
  }

  public highlightSearch(query: string): HTMLElement[] {
    const matches: HTMLElement[] = [];
    const lowerQuery = query.toLowerCase();
    const children = this.container.children;

    for (let i = 0; i < children.length; i++) {
      const el = children[i] as HTMLElement;
      el.classList.remove('highlight-active');
      if (query.trim() && el.textContent?.toLowerCase().includes(lowerQuery)) {
        el.classList.add('highlight-search');
        matches.push(el);
      } else {
        el.classList.remove('highlight-search');
      }
    }
    return matches;
  }

  public clearHighlights() {
    const children = this.container.children;
    for (let i = 0; i < children.length; i++) {
      const el = children[i] as HTMLElement;
      el.classList.remove('highlight-search', 'highlight-active');
    }
  }
}

