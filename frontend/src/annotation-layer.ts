export interface ClientAnnotation {
  id: string;
  page_number: number;
  annotation_type: 'highlight' | 'note' | 'rectangle' | 'freehand' | 'redact';
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  opacity: number;
  author: string;
  created_at: string;
  content?: string;
  reason?: string;
}

export class AnnotationLayerManager {
  private container: HTMLElement;
  private pageCard: HTMLElement;
  private pageNumber: number;
  private scale: number = 1.0;
  private currentTool: string = 'select';
  private annotations: ClientAnnotation[] = [];
  private isMouseDown: boolean = false;
  private startX: number = 0;
  private startY: number = 0;
  private previewBox: HTMLElement | null = null;
  private activePopover: HTMLElement | null = null;

  private onAnnotationCreated?: (ann: ClientAnnotation) => void;
  private onAnnotationDeleted?: (id: string) => void;
  private onAnnotationUpdated?: (ann: ClientAnnotation) => void;

  constructor(
    container: HTMLElement,
    pageNumber: number,
    onAnnotationCreated?: (ann: ClientAnnotation) => void,
    onAnnotationDeleted?: (id: string) => void,
    onAnnotationUpdated?: (ann: ClientAnnotation) => void
  ) {
    this.container = container;
    this.pageCard = (container.parentElement || container) as HTMLElement;
    this.pageNumber = pageNumber;
    this.onAnnotationCreated = onAnnotationCreated;
    this.onAnnotationDeleted = onAnnotationDeleted;
    this.onAnnotationUpdated = onAnnotationUpdated;

    // Container itself never blocks pointer events so text layer underneath remains selectable
    this.container.style.pointerEvents = 'none';

    this.setupEvents();
    this.setTool(this.currentTool);
  }

  public setTool(tool: string) {
    this.currentTool = tool;
    this.closeActivePopover();

    if (this.currentTool === 'note') {
      this.pageCard.style.cursor = 'crosshair';
    } else if (this.currentTool === 'highlight') {
      this.pageCard.style.cursor = 'text';
    } else if (this.currentTool === 'redact') {
      this.pageCard.style.cursor = 'crosshair';
    } else {
      this.pageCard.style.cursor = 'default';
    }
  }

  public setScale(scale: number) {
    this.scale = scale;
    this.redraw();
  }

  public setAnnotations(annots: ClientAnnotation[]) {
    this.annotations = annots.filter((a) => a.page_number === this.pageNumber);
    this.redraw();
  }

  public getAnnotations(): ClientAnnotation[] {
    return this.annotations;
  }

  public addAnnotation(ann: ClientAnnotation) {
    this.annotations.push(ann);
    this.redraw();
    this.onAnnotationCreated?.(ann);
  }

  public removeAnnotation(id: string) {
    this.annotations = this.annotations.filter((a) => a.id !== id);
    this.redraw();
    this.onAnnotationDeleted?.(id);
  }

  public updateAnnotation(ann: ClientAnnotation) {
    const idx = this.annotations.findIndex((a) => a.id === ann.id);
    if (idx !== -1) {
      this.annotations[idx] = ann;
      this.redraw();
      this.onAnnotationUpdated?.(ann);
    }
  }

  private closeActivePopover() {
    if (this.activePopover) {
      this.activePopover.remove();
      this.activePopover = null;
    }
  }

  private setupEvents() {
    this.pageCard.addEventListener('mousedown', (e: MouseEvent) => {
      // Don't trigger new action if clicking inside an existing popover
      if ((e.target as HTMLElement).closest('.annot-popover-card') || (e.target as HTMLElement).closest('.annot-delete-pill')) {
        return;
      }
      this.closeActivePopover();

      if (this.currentTool === 'select') return;

      const cardRect = this.pageCard.getBoundingClientRect();
      this.startX = e.clientX - cardRect.left;
      this.startY = e.clientY - cardRect.top;

      if (this.currentTool === 'note') {
        // Handled on click / mouseup to avoid accidental drags
        return;
      }

      if (this.currentTool === 'highlight' || this.currentTool === 'redact') {
        this.isMouseDown = true;
        this.previewBox = document.createElement('div');
        this.previewBox.className = this.currentTool === 'redact' ? 'annot-preview-redact' : 'annot-preview-highlight';
        this.previewBox.style.left = `${this.startX}px`;
        this.previewBox.style.top = `${this.startY}px`;
        this.pageCard.appendChild(this.previewBox);
      }
    });

    window.addEventListener('mousemove', (e: MouseEvent) => {
      if (!this.isMouseDown || !this.previewBox) return;

      const cardRect = this.pageCard.getBoundingClientRect();
      const currX = Math.max(0, Math.min(cardRect.width, e.clientX - cardRect.left));
      const currY = Math.max(0, Math.min(cardRect.height, e.clientY - cardRect.top));

      const left = Math.min(this.startX, currX);
      const top = Math.min(this.startY, currY);
      const width = Math.abs(currX - this.startX);
      const height = Math.abs(currY - this.startY);

      this.previewBox.style.left = `${left}px`;
      this.previewBox.style.top = `${top}px`;
      this.previewBox.style.width = `${width}px`;
      this.previewBox.style.height = `${height}px`;
    });

    window.addEventListener('mouseup', (e: MouseEvent) => {
      if (this.currentTool === 'note') {
        const cardRect = this.pageCard.getBoundingClientRect();
        // Ensure the click was actually inside this page
        if (
          e.clientX >= cardRect.left &&
          e.clientX <= cardRect.right &&
          e.clientY >= cardRect.top &&
          e.clientY <= cardRect.bottom
        ) {
          // If not clicked on existing pin
          if (!(e.target as HTMLElement).closest('.annot-note-pin') && !(e.target as HTMLElement).closest('.annot-popover-card')) {
            const clickX = e.clientX - cardRect.left;
            const clickY = e.clientY - cardRect.top;
            this.openNoteCreatePopover(clickX, clickY);
          }
        }
        return;
      }

      if (!this.isMouseDown) return;
      this.isMouseDown = false;

      if (this.previewBox) {
        this.previewBox.remove();
        this.previewBox = null;
      }

      // Check if text was selected in DOM
      const selection = window.getSelection();
      const selectedText = selection ? selection.toString().trim() : '';

      if (selectedText.length > 0 && selection && selection.rangeCount > 0) {
        const range = selection.getRangeAt(0);
        this.createAnnotationsFromSelectionRange(range, this.currentTool as 'highlight' | 'redact');
        selection.removeAllRanges();
        return;
      }

      // If no text was selected, check if user dragged a rectangular box
      const cardRect = this.pageCard.getBoundingClientRect();
      const currX = Math.max(0, Math.min(cardRect.width, e.clientX - cardRect.left));
      const currY = Math.max(0, Math.min(cardRect.height, e.clientY - cardRect.top));

      const left = Math.min(this.startX, currX);
      const top = Math.min(this.startY, currY);
      const width = Math.abs(currX - this.startX);
      const height = Math.abs(currY - this.startY);

      if (width > 8 && height > 8) {
        const annType = this.currentTool === 'redact' ? 'redact' : 'highlight';
        const ann: ClientAnnotation = {
          id: crypto.randomUUID(),
          page_number: this.pageNumber,
          annotation_type: annType,
          x: left / this.scale,
          y: top / this.scale,
          width: width / this.scale,
          height: height / this.scale,
          color: annType === 'redact' ? '#000000' : '#fef08a',
          opacity: annType === 'redact' ? 1.0 : 0.45,
          author: 'Utilisateur',
          created_at: new Date().toISOString(),
          reason: annType === 'redact' ? 'RGPD' : undefined,
        };

        this.addAnnotation(ann);
      }
    });
  }

  public createAnnotationsFromSelectionRange(range: Range, type: 'highlight' | 'redact'): ClientAnnotation[] {
    const cardRect = this.pageCard.getBoundingClientRect();
    const rawRects = Array.from(range.getClientRects());
    const created: ClientAnnotation[] = [];

    // Filter rects inside this page card and convert to page point coordinates
    const pageRects: { x: number; y: number; width: number; height: number }[] = [];

    for (const r of rawRects) {
      if (r.right <= cardRect.left || r.left >= cardRect.right || r.bottom <= cardRect.top || r.top >= cardRect.bottom) {
        continue;
      }
      if (r.width < 2 || r.height < 2) continue;

      const rx = Math.max(0, r.left - cardRect.left) / this.scale;
      const ry = Math.max(0, r.top - cardRect.top) / this.scale;
      const rw = r.width / this.scale;
      const rh = r.height / this.scale;

      pageRects.push({ x: rx, y: ry, width: rw, height: rh });
    }

    // Merge rects on the same line to avoid overlapping fragmented boxes
    const mergedRects = this.mergeLineRects(pageRects);

    for (const rect of mergedRects) {
      const ann: ClientAnnotation = {
        id: crypto.randomUUID(),
        page_number: this.pageNumber,
        annotation_type: type,
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        color: type === 'redact' ? '#000000' : '#fef08a',
        opacity: type === 'redact' ? 1.0 : 0.45,
        author: 'Utilisateur',
        created_at: new Date().toISOString(),
        reason: type === 'redact' ? 'RGPD' : undefined,
      };

      this.annotations.push(ann);
      created.push(ann);
      this.onAnnotationCreated?.(ann);
    }

    if (created.length > 0) {
      this.redraw();
    }

    return created;
  }

  private mergeLineRects(rects: { x: number; y: number; width: number; height: number }[]) {
    if (rects.length <= 1) return rects;

    // Sort by y then x
    rects.sort((a, b) => (Math.abs(a.y - b.y) < 4 ? a.x - b.x : a.y - b.y));

    const merged: { x: number; y: number; width: number; height: number }[] = [];
    let curr = { ...rects[0] };

    for (let i = 1; i < rects.length; i++) {
      const next = rects[i];
      const sameLine = Math.abs(curr.y - next.y) < 6 && Math.abs(curr.height - next.height) < 6;
      const overlapsOrTouches = next.x <= curr.x + curr.width + 6;

      if (sameLine && overlapsOrTouches) {
        const right = Math.max(curr.x + curr.width, next.x + next.width);
        curr.width = right - curr.x;
      } else {
        merged.push(curr);
        curr = { ...next };
      }
    }
    merged.push(curr);
    return merged;
  }

  public openNoteCreatePopover(clickX: number, clickY: number, initialContent?: string) {
    this.closeActivePopover();

    const popover = document.createElement('div');
    popover.className = 'annot-popover-card';
    popover.style.left = `${Math.min(clickX, this.pageCard.clientWidth - 260)}px`;
    popover.style.top = `${Math.min(clickY, this.pageCard.clientHeight - 180)}px`;

    popover.innerHTML = `
      <div class="annot-popover-header">
        <div class="annot-popover-title">📝 Nouvelle note</div>
        <button class="annot-popover-close">✕</button>
      </div>
      <textarea class="annot-popover-input" placeholder="Saisissez votre note ou commentaire..." rows="3">${initialContent || ''}</textarea>
      <div class="annot-popover-actions">
        <button class="annot-btn-cancel">Annuler</button>
        <button class="annot-btn-save">Enregistrer</button>
      </div>
    `;

    const closeBtn = popover.querySelector('.annot-popover-close') as HTMLElement;
    const cancelBtn = popover.querySelector('.annot-btn-cancel') as HTMLElement;
    const saveBtn = popover.querySelector('.annot-btn-save') as HTMLElement;
    const textarea = popover.querySelector('.annot-popover-input') as HTMLTextAreaElement;

    closeBtn.onclick = (e) => { e.stopPropagation(); this.closeActivePopover(); };
    cancelBtn.onclick = (e) => { e.stopPropagation(); this.closeActivePopover(); };

    saveBtn.onclick = (e) => {
      e.stopPropagation();
      const content = textarea.value.trim();
      if (content) {
        const ann: ClientAnnotation = {
          id: crypto.randomUUID(),
          page_number: this.pageNumber,
          annotation_type: 'note',
          x: clickX / this.scale,
          y: clickY / this.scale,
          width: 26,
          height: 26,
          color: '#f59e0b',
          opacity: 1.0,
          author: 'Utilisateur',
          created_at: new Date().toISOString(),
          content: content,
        };
        this.addAnnotation(ann);
      }
      this.closeActivePopover();
    };

    this.pageCard.appendChild(popover);
    this.activePopover = popover;
    setTimeout(() => textarea.focus(), 50);
  }

  private openNoteViewPopover(ann: ClientAnnotation, pinEl: HTMLElement) {
    this.closeActivePopover();

    const pinRect = pinEl.getBoundingClientRect();
    const cardRect = this.pageCard.getBoundingClientRect();
    const posX = pinRect.left - cardRect.left;
    const posY = pinRect.top - cardRect.top + 30;

    const popover = document.createElement('div');
    popover.className = 'annot-popover-card';
    popover.style.left = `${Math.min(posX, this.pageCard.clientWidth - 260)}px`;
    popover.style.top = `${Math.min(posY, this.pageCard.clientHeight - 180)}px`;

    const dateStr = new Date(ann.created_at).toLocaleDateString('fr-FR', {
      day: 'numeric',
      month: 'short',
      hour: '2-digit',
      minute: '2-digit',
    });

    popover.innerHTML = `
      <div class="annot-popover-header">
        <div class="annot-popover-title">📝 Note (${this.escapeHtml(ann.author)})</div>
        <button class="annot-popover-close">✕</button>
      </div>
      <div class="annot-popover-meta">${dateStr}</div>
      <div class="annot-popover-body">${this.escapeHtml(ann.content || '(Vide)')}</div>
      <div class="annot-popover-actions">
        <button class="annot-btn-delete">🗑 Supprimer</button>
        <button class="annot-btn-edit">✏️ Modifier</button>
      </div>
    `;

    const closeBtn = popover.querySelector('.annot-popover-close') as HTMLElement;
    const deleteBtn = popover.querySelector('.annot-btn-delete') as HTMLElement;
    const editBtn = popover.querySelector('.annot-btn-edit') as HTMLElement;

    closeBtn.onclick = (e) => { e.stopPropagation(); this.closeActivePopover(); };

    deleteBtn.onclick = (e) => {
      e.stopPropagation();
      this.removeAnnotation(ann.id);
      this.closeActivePopover();
    };

    editBtn.onclick = (e) => {
      e.stopPropagation();
      const currentText = ann.content || '';
      popover.innerHTML = `
        <div class="annot-popover-header">
          <div class="annot-popover-title">✏️ Modifier la note</div>
          <button class="annot-popover-close">✕</button>
        </div>
        <textarea class="annot-popover-input" rows="3">${this.escapeHtml(currentText)}</textarea>
        <div class="annot-popover-actions">
          <button class="annot-btn-cancel">Annuler</button>
          <button class="annot-btn-save">Enregistrer</button>
        </div>
      `;
      const edClose = popover.querySelector('.annot-popover-close') as HTMLElement;
      const edCancel = popover.querySelector('.annot-btn-cancel') as HTMLElement;
      const edSave = popover.querySelector('.annot-btn-save') as HTMLElement;
      const textarea = popover.querySelector('.annot-popover-input') as HTMLTextAreaElement;

      edClose.onclick = (ev) => { ev.stopPropagation(); this.closeActivePopover(); };
      edCancel.onclick = (ev) => { ev.stopPropagation(); this.closeActivePopover(); };

      edSave.onclick = (ev) => {
        ev.stopPropagation();
        ann.content = textarea.value.trim();
        this.updateAnnotation(ann);
        this.closeActivePopover();
      };
      setTimeout(() => textarea.focus(), 50);
    };

    this.pageCard.appendChild(popover);
    this.activePopover = popover;
  }

  private showDeletePill(ann: ClientAnnotation, targetEl: HTMLElement) {
    this.closeActivePopover();

    const pill = document.createElement('div');
    pill.className = 'annot-delete-pill';
    const rect = targetEl.getBoundingClientRect();
    const cardRect = this.pageCard.getBoundingClientRect();

    pill.style.left = `${rect.left - cardRect.left + (rect.width / 2) - 40}px`;
    pill.style.top = `${rect.top - cardRect.top - 32}px`;
    pill.innerHTML = `<span>🗑 Supprimer</span>`;

    pill.onclick = (e) => {
      e.stopPropagation();
      this.removeAnnotation(ann.id);
      pill.remove();
    };

    this.pageCard.appendChild(pill);
    this.activePopover = pill;

    // Auto-remove pill on outside click
    const outsideListener = (ev: MouseEvent) => {
      if (!pill.contains(ev.target as Node)) {
        pill.remove();
        document.removeEventListener('mousedown', outsideListener);
      }
    };
    setTimeout(() => document.addEventListener('mousedown', outsideListener), 50);
  }

  private escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }

  public redraw() {
    this.container.innerHTML = '';

    for (const ann of this.annotations) {
      const el = document.createElement('div');
      const left = ann.x * this.scale;
      const top = ann.y * this.scale;
      const w = ann.width * this.scale;
      const h = ann.height * this.scale;

      el.style.left = `${left}px`;
      el.style.top = `${top}px`;

      if (ann.annotation_type === 'note') {
        el.className = 'annot-note-pin';
        el.title = `${ann.author}: ${ann.content || ''}`;
        el.textContent = '📝';
        el.onclick = (ev) => {
          ev.stopPropagation();
          this.openNoteViewPopover(ann, el);
        };
      } else if (ann.annotation_type === 'redact') {
        el.className = 'annot-redaction';
        el.style.width = `${w}px`;
        el.style.height = `${h}px`;
        el.textContent = ann.reason || 'BIFFÉ';
        el.title = 'Cliquez pour supprimer cette biffure';
        el.onclick = (ev) => {
          ev.stopPropagation();
          this.showDeletePill(ann, el);
        };
      } else {
        el.className = 'annot-highlight';
        el.style.width = `${w}px`;
        el.style.height = `${h}px`;
        el.title = 'Cliquez pour supprimer ce surlignage';
        el.onclick = (ev) => {
          ev.stopPropagation();
          this.showDeletePill(ann, el);
        };
      }

      this.container.appendChild(el);
    }
  }
}
