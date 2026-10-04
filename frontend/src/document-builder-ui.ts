export interface BuilderPageMeta {
  page_number: number;
  width: number;
  height: number;
  rotation?: number;
}

export interface BuilderPageItem {
  pageNumber: number;
  width: number;
  height: number;
  rotation: number;
  deleted: boolean;
}

export class DocumentBuilderUI {
  private docId: string;
  private pages: BuilderPageItem[] = [];
  private onDocumentBuilt: (newDocId: string) => void;

  constructor(
    docId: string,
    pagesInput: number | BuilderPageMeta[],
    onDocumentBuilt: (newDocId: string) => void
  ) {
    this.docId = docId;
    this.onDocumentBuilt = onDocumentBuilt;

    if (Array.isArray(pagesInput)) {
      for (const p of pagesInput) {
        this.pages.push({
          pageNumber: p.page_number,
          width: p.width || 595,
          height: p.height || 842,
          rotation: 0,
          deleted: false,
        });
      }
    } else {
      for (let i = 1; i <= pagesInput; i++) {
        this.pages.push({
          pageNumber: i,
          width: 595,
          height: 842,
          rotation: 0,
          deleted: false,
        });
      }
    }
  }

  public render(container: HTMLElement) {
    container.innerHTML = '';

    const activeCount = this.pages.filter((p) => !p.deleted).length;

    for (const p of this.pages) {
      const item = document.createElement('div');
      item.className = 'builder-item';
      if (p.deleted) {
        item.classList.add('builder-item-deleted');
      }

      // Calculate orientation & dimensions
      const totalRotation = p.rotation % 360;
      const isTurned90or270 = totalRotation === 90 || totalRotation === 270;
      const baseRatio = (p.width && p.height) ? (p.width / p.height) : 0.707;
      const visualRatio = isTurned90or270 ? (1 / baseRatio) : baseRatio;

      // Max thumbnail bounding box: 150px wide, 160px tall
      let boxW = 120;
      let boxH = 120;
      if (visualRatio >= 1) {
        // Landscape
        boxW = 150;
        boxH = Math.max(70, Math.round(150 / visualRatio));
      } else {
        // Portrait
        boxH = 160;
        boxW = Math.max(70, Math.round(160 * visualRatio));
      }

      const thumb = document.createElement('div');
      thumb.className = 'builder-thumb';
      thumb.style.width = `${boxW}px`;
      thumb.style.height = `${boxH}px`;
      thumb.style.position = 'relative';

      const img = document.createElement('img');
      img.src = `/api/documents/${this.docId}/pages/${p.pageNumber}/thumbnail`;
      img.alt = `Page ${p.pageNumber}`;
      img.loading = 'lazy';
      img.style.transition = 'transform 0.2s ease';

      if (isTurned90or270) {
        // Image rotated 90/270deg: width & height swapped
        img.style.width = `${boxH}px`;
        img.style.height = `${boxW}px`;
        img.style.transform = `rotate(${totalRotation}deg)`;
        img.style.position = 'absolute';
      } else {
        img.style.width = `${boxW}px`;
        img.style.height = `${boxH}px`;
        img.style.transform = totalRotation === 180 ? 'rotate(180deg)' : 'none';
        img.style.position = 'relative';
      }
      thumb.appendChild(img);

      // Deleted overlay
      if (p.deleted) {
        const delOverlay = document.createElement('div');
        delOverlay.className = 'builder-deleted-overlay';
        delOverlay.innerHTML = '<span>🗑️ Supprimée</span>';
        thumb.appendChild(delOverlay);
      }

      const labelRow = document.createElement('div');
      labelRow.className = 'builder-label-row';

      const label = document.createElement('span');
      label.className = 'builder-page-label';
      label.textContent = `Page ${p.pageNumber}`;
      labelRow.appendChild(label);

      if (p.rotation > 0) {
        const rotBadge = document.createElement('span');
        rotBadge.className = 'builder-rot-badge';
        rotBadge.textContent = `↻ ${p.rotation}°`;
        labelRow.appendChild(rotBadge);
      }

      const actions = document.createElement('div');
      actions.className = 'builder-actions';

      if (!p.deleted) {
        const rotBtn = document.createElement('button');
        rotBtn.className = 'builder-btn';
        rotBtn.textContent = '↻ 90°';
        rotBtn.title = 'Faire pivoter de 90° dans le sens horaire';
        rotBtn.onclick = () => {
          p.rotation = (p.rotation + 90) % 360;
          this.render(container);
        };

        const delBtn = document.createElement('button');
        delBtn.className = 'builder-btn builder-btn-danger';
        delBtn.textContent = '🗑';
        delBtn.title = 'Supprimer cette page';
        delBtn.onclick = () => {
          p.deleted = true;
          this.render(container);
        };

        actions.appendChild(rotBtn);
        actions.appendChild(delBtn);
      } else {
        const restoreBtn = document.createElement('button');
        restoreBtn.className = 'builder-btn builder-btn-restore';
        restoreBtn.textContent = '↩ Restaurer';
        restoreBtn.title = 'Restaurer cette page dans le document';
        restoreBtn.onclick = () => {
          p.deleted = false;
          this.render(container);
        };
        actions.appendChild(restoreBtn);
      }

      item.appendChild(thumb);
      item.appendChild(labelRow);
      item.appendChild(actions);

      container.appendChild(item);
    }
  }

  public async applyBuild(watermarkText?: string): Promise<void> {
    const activePages = this.pages.filter((p) => !p.deleted);
    if (activePages.length === 0) {
      alert('Toutes les pages ont été supprimées. Au moins une page doit être conservée.');
      return;
    }

    const pageActions: any[] = [];

    for (const p of this.pages) {
      if (p.deleted) {
        pageActions.push({
          action: 'delete',
          page_number: p.pageNumber,
        });
      } else if (p.rotation !== 0) {
        pageActions.push({
          action: 'rotate',
          page_number: p.pageNumber,
          degrees: p.rotation,
        });
      }
    }

    const payload = {
      source_document_ids: [this.docId],
      page_actions: pageActions,
      watermark: watermarkText?.trim()
        ? {
            text: watermarkText.trim(),
            opacity: 0.3,
            font_size: 36,
            rotation: 45,
            color: '#888888',
          }
        : null,
    };

    try {
      const resp = await fetch('/api/documents/build', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!resp.ok) {
        const err = await resp.text();
        alert(`Erreur lors de la génération du document : ${err}`);
        return;
      }

      const meta = await resp.json();
      alert(`Document assemblé avec succès ! (${meta.page_count} page(s), ID: ${meta.id})`);
      this.onDocumentBuilt(meta.id);
    } catch (e) {
      alert(`Erreur: ${e}`);
    }
  }
}
