import { DocumentMetadata } from './viewer.ts';

export interface PageDiffData {
  imgUrl: string;
  diffRatio: number;
  hasDiff: boolean;
}

export interface TextDiffToken {
  op: 'equal' | 'insert' | 'delete';
  text: string;
}

export interface TextDiffResult {
  page_number: number;
  additions_count: number;
  deletions_count: number;
  unchanged_count: number;
  diff_ratio: number;
  tokens: TextDiffToken[];
}

export class ComparisonUI {
  private docA: DocumentMetadata;
  private modal: HTMLElement;
  private container: HTMLElement;
  private statsEl: HTMLElement;

  private docBMeta: any | null = null;
  private currentPage: number = 1;
  private totalPages: number = 1;
  private diffCache: Map<number, PageDiffData> = new Map();
  private textDiffCache: Map<number, TextDiffResult> = new Map();
  private isContinuous: boolean = false;
  private isScanning: boolean = false;
  private diffMode: 'visual' | 'text' = 'visual';

  constructor(docA: DocumentMetadata, modal: HTMLElement, container: HTMLElement, statsEl: HTMLElement) {
    this.docA = docA;
    this.modal = modal;
    this.container = container;
    this.statsEl = statsEl;
  }

  public async runComparison(file: File): Promise<void> {
    this.statsEl.textContent = 'Téléversement du second document...';
    this.diffCache.clear();
    this.textDiffCache.clear();
    this.currentPage = 1;

    const navToolbar = this.modal.querySelector('#compareNavToolbar') as HTMLElement;
    if (navToolbar) navToolbar.style.display = 'none';

    // 1. Téléversement du document B
    const formData = new FormData();
    formData.append('file', file);

    try {
      const uploadResp = await fetch('/api/documents', {
        method: 'POST',
        body: formData,
      });

      if (!uploadResp.ok) {
        this.statsEl.textContent = 'Échec du téléversement du second document.';
        return;
      }

      this.docBMeta = await uploadResp.json();
      this.totalPages = Math.max(this.docA.page_count || 1, this.docBMeta.page_count || 1);

      this.setupNavToolbar();

      // Charge la première page
      await this.goToPage(1);

      // Analyse automatiquement en arrière-plan les pages restantes pour afficher les pastilles de synthèse
      if (this.totalPages > 1) {
        this.scanAllPages();
      }
    } catch (e) {
      this.statsEl.textContent = `Erreur: ${e}`;
    }
  }

  private setupNavToolbar() {
    const navToolbar = this.modal.querySelector('#compareNavToolbar') as HTMLElement;
    if (!navToolbar) return;

    navToolbar.style.display = 'flex';

    const totalLabel = this.modal.querySelector('#compareTotalPagesLabel');
    if (totalLabel) totalLabel.textContent = this.totalPages.toString();

    const prevBtn = this.modal.querySelector('#btnComparePrevPage') as HTMLButtonElement;
    const nextBtn = this.modal.querySelector('#btnCompareNextPage') as HTMLButtonElement;
    const scanBtn = this.modal.querySelector('#btnCompareAllPages') as HTMLButtonElement;
    const contBtn = this.modal.querySelector('#btnToggleContinuousDiff') as HTMLButtonElement;

    const btnVis = this.modal.querySelector('#btnModeVisualDiff') as HTMLButtonElement;
    const btnTxt = this.modal.querySelector('#btnModeTextDiff') as HTMLButtonElement;

    if (btnVis && btnTxt) {
      btnVis.onclick = () => {
        if (this.diffMode !== 'visual') {
          this.diffMode = 'visual';
          btnVis.classList.add('active');
          btnTxt.classList.remove('active');
          this.goToPage(this.currentPage);
        }
      };
      btnTxt.onclick = () => {
        if (this.diffMode !== 'text') {
          this.diffMode = 'text';
          btnTxt.classList.add('active');
          btnVis.classList.remove('active');
          this.goToPage(this.currentPage);
        }
      };
    }

    if (prevBtn) prevBtn.onclick = () => this.goToPage(this.currentPage - 1);
    if (nextBtn) nextBtn.onclick = () => this.goToPage(this.currentPage + 1);
    if (scanBtn) scanBtn.onclick = () => this.scanAllPages();
    if (contBtn) {
      contBtn.onclick = () => this.toggleContinuous();
      contBtn.textContent = this.isContinuous ? 'Vue Page par Page' : 'Vue continue';
    }

    this.renderPageChips();
  }

  private renderPageChips() {
    const chipsContainer = this.modal.querySelector('#comparePageChips') as HTMLElement;
    if (!chipsContainer) return;

    chipsContainer.innerHTML = '';

    for (let p = 1; p <= this.totalPages; p++) {
      const chip = document.createElement('button');
      chip.className = 'btn';
      chip.style.fontSize = '11px';
      chip.style.padding = '3px 8px';
      chip.style.display = 'flex';
      chip.style.alignItems = 'center';
      chip.style.gap = '4px';
      chip.dataset.page = p.toString();

      if (p === this.currentPage && !this.isContinuous) {
        chip.style.borderColor = 'var(--accent)';
        chip.style.background = 'var(--accent)';
        chip.style.color = 'white';
      }

      const cached = this.diffCache.get(p);
      let badge = '';
      if (cached) {
        const pct = (cached.diffRatio * 100).toFixed(1);
        badge = cached.hasDiff ? ` <span style="color: #f87171; font-weight: bold;">(${pct}%)</span>` : ' <span style="color: #34d399;">(0%)</span>';
      }

      chip.innerHTML = `P.${p}${badge}`;
      chip.onclick = () => {
        if (this.isContinuous) {
          this.isContinuous = false;
          const contBtn = this.modal.querySelector('#btnToggleContinuousDiff') as HTMLButtonElement;
          if (contBtn) contBtn.textContent = 'Vue continue';
        }
        this.goToPage(p);
      };

      chipsContainer.appendChild(chip);
    }
  }

  public async goToPage(pageNum: number): Promise<void> {
    if (pageNum < 1 || pageNum > this.totalPages) return;
    this.currentPage = pageNum;

    const currLabel = this.modal.querySelector('#compareCurrentPageLabel');
    if (currLabel) currLabel.textContent = pageNum.toString();

    const prevBtn = this.modal.querySelector('#btnComparePrevPage') as HTMLButtonElement;
    const nextBtn = this.modal.querySelector('#btnCompareNextPage') as HTMLButtonElement;
    if (prevBtn) prevBtn.disabled = pageNum <= 1;
    if (nextBtn) nextBtn.disabled = pageNum >= this.totalPages;

    this.renderPageChips();

    if (this.isContinuous) {
      this.renderContinuousView();
    } else {
      if (this.diffMode === 'text') {
        await this.loadPageTextDiff(pageNum);
      } else {
        await this.loadPageDiff(pageNum);
      }
    }
  }

  private async fetchPageDiff(pageNum: number): Promise<PageDiffData | null> {
    if (this.diffCache.has(pageNum)) {
      return this.diffCache.get(pageNum)!;
    }

    try {
      const resp = await fetch('/api/documents/compare', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          doc_a_id: this.docA.id,
          doc_b_id: this.docBMeta.id,
          page_number: pageNum,
          mode: 'visual',
        }),
      });

      if (!resp.ok) return null;

      const diffRatio = parseFloat(resp.headers.get('X-Diff-Ratio') || '0');
      const hasDiff = resp.headers.get('X-Has-Differences') === 'true';
      const blob = await resp.blob();
      const imgUrl = URL.createObjectURL(blob);

      const data: PageDiffData = { imgUrl, diffRatio, hasDiff };
      this.diffCache.set(pageNum, data);
      return data;
    } catch {
      return null;
    }
  }

  private async fetchTextDiff(pageNum: number): Promise<TextDiffResult | null> {
    if (this.textDiffCache.has(pageNum)) {
      return this.textDiffCache.get(pageNum)!;
    }

    try {
      const resp = await fetch('/api/documents/compare', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          doc_a_id: this.docA.id,
          doc_b_id: this.docBMeta.id,
          page_number: pageNum,
          mode: 'text',
        }),
      });

      if (!resp.ok) return null;
      const data: TextDiffResult = await resp.json();
      this.textDiffCache.set(pageNum, data);
      return data;
    } catch {
      return null;
    }
  }

  public async loadPageDiff(pageNum: number): Promise<void> {
    this.statsEl.textContent = `Calcul du différentiel visuel page ${pageNum} / ${this.totalPages}...`;

    const data = await this.fetchPageDiff(pageNum);
    if (!data) {
      this.statsEl.textContent = `Erreur lors de la comparaison visuelle de la page ${pageNum}.`;
      return;
    }

    const pct = (data.diffRatio * 100).toFixed(2);
    this.statsEl.textContent = data.hasDiff
      ? `Page ${pageNum} : ⚠️ ${pct}% de variation (Vert = Ajouté, Rouge = Supprimé)`
      : `Page ${pageNum} : ✅ 100% identique (0% de différence)`;
    this.statsEl.style.color = data.hasDiff ? 'var(--warning)' : 'var(--success)';

    this.container.innerHTML = `
      <div style="display: flex; flex-direction: column; align-items: center; gap: 10px;">
        <img src="${data.imgUrl}" style="max-width: 100%; border-radius: 4px; box-shadow: 0 4px 12px rgba(0,0,0,0.5);" alt="Diff page ${pageNum}">
      </div>
    `;

    this.renderPageChips();
  }

  public async loadPageTextDiff(pageNum: number): Promise<void> {
    this.statsEl.textContent = `Calcul du différentiel sémantique page ${pageNum} / ${this.totalPages}...`;

    const data = await this.fetchTextDiff(pageNum);
    if (!data) {
      this.statsEl.textContent = `Erreur lors de l'analyse sémantique de la page ${pageNum}.`;
      return;
    }

    const hasDiff = data.additions_count + data.deletions_count > 0;
    const pct = (data.diff_ratio * 100).toFixed(1);
    this.statsEl.textContent = hasDiff
      ? `Page ${pageNum} : ⚠️ Diff Sémantique (${pct}% de variation, +${data.additions_count} mots ajoutés, -${data.deletions_count} mots supprimés)`
      : `Page ${pageNum} : ✅ 100% de concordance sémantique (aucun mot modifié)`;
    this.statsEl.style.color = hasDiff ? 'var(--warning)' : 'var(--success)';

    let htmlTokens = '';
    for (const t of data.tokens) {
      const escaped = t.text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\n/g, '<br/>');
      if (t.op === 'insert') {
        htmlTokens += `<ins style="background: rgba(34, 197, 94, 0.25); color: #4ade80; text-decoration: none; padding: 2px 5px; border-radius: 4px; font-weight: 600; border-bottom: 2px solid #22c55e;">${escaped}</ins>`;
      } else if (t.op === 'delete') {
        htmlTokens += `<del style="background: rgba(239, 68, 68, 0.25); color: #f87171; text-decoration: line-through; padding: 2px 5px; border-radius: 4px; font-weight: 500; border-bottom: 2px solid #ef4444;">${escaped}</del>`;
      } else {
        htmlTokens += `<span style="color: #cbd5e1;">${escaped}</span>`;
      }
    }

    this.container.innerHTML = `
      <div style="max-width: 850px; margin: 0 auto; background: var(--bg-primary, #0f172a); border: 1px solid var(--border); border-radius: 8px; padding: 24px; text-align: left; box-shadow: 0 4px 16px rgba(0,0,0,0.4);">
        <div style="display: flex; gap: 12px; margin-bottom: 18px; padding-bottom: 12px; border-bottom: 1px solid var(--border); flex-wrap: wrap;">
          <span style="font-size: 12px; background: rgba(34, 197, 94, 0.15); color: #4ade80; padding: 3px 10px; border-radius: 12px; border: 1px solid rgba(34, 197, 94, 0.3); font-weight: 600;">
            +${data.additions_count} ajout(s)
          </span>
          <span style="font-size: 12px; background: rgba(239, 68, 68, 0.15); color: #f87171; padding: 3px 10px; border-radius: 12px; border: 1px solid rgba(239, 68, 68, 0.3); font-weight: 600;">
            -${data.deletions_count} suppression(s)
          </span>
          <span style="font-size: 12px; background: rgba(148, 163, 184, 0.15); color: #cbd5e1; padding: 3px 10px; border-radius: 12px;">
            ${data.unchanged_count} mot(s) inchangé(s)
          </span>
        </div>
        <div style="font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace; font-size: 14px; line-height: 1.8; white-space: pre-wrap; word-break: break-word;">
          ${htmlTokens || '<em style="color: #64748b;">(Page sans texte détecté)</em>'}
        </div>
      </div>
    `;

    this.renderPageChips();
  }

  public async scanAllPages(): Promise<void> {
    if (this.isScanning) return;
    this.isScanning = true;

    const scanBtn = this.modal.querySelector('#btnCompareAllPages') as HTMLButtonElement;
    if (scanBtn) {
      scanBtn.disabled = true;
      scanBtn.textContent = '⏳ Scan en cours...';
    }

    let diffCount = 0;
    for (let p = 1; p <= this.totalPages; p++) {
      const data = await this.fetchPageDiff(p);
      if (data && data.hasDiff) {
        diffCount++;
      }
      this.renderPageChips();
    }

    if (scanBtn) {
      scanBtn.disabled = false;
      scanBtn.textContent = '⚡ Re-scanner';
    }
    this.isScanning = false;

    if (diffCount > 0) {
      this.statsEl.textContent = `⚠️ Différences détectées sur ${diffCount} page(s) sur ${this.totalPages} au total.`;
      this.statsEl.style.color = 'var(--warning)';
    } else {
      this.statsEl.textContent = `✅ Document entier identique : 0% de différence sur les ${this.totalPages} page(s).`;
      this.statsEl.style.color = 'var(--success)';
    }

    if (this.isContinuous) {
      this.renderContinuousView();
    }
  }

  public toggleContinuous() {
    this.isContinuous = !this.isContinuous;
    const contBtn = this.modal.querySelector('#btnToggleContinuousDiff') as HTMLButtonElement;
    if (contBtn) {
      contBtn.textContent = this.isContinuous ? 'Vue Page par Page' : 'Vue continue';
    }

    if (this.isContinuous) {
      this.renderContinuousView();
    } else {
      this.goToPage(this.currentPage);
    }
  }

  private async renderContinuousView() {
    this.container.innerHTML = '<div style="color: #bbb; padding: 20px;">Chargement de toutes les pages...</div>';
    const frag = document.createDocumentFragment();

    for (let p = 1; p <= this.totalPages; p++) {
      const pageWrapper = document.createElement('div');
      pageWrapper.style.cssText = 'margin-bottom: 24px; display: flex; flex-direction: column; align-items: center; gap: 8px;';

      const pageHeader = document.createElement('div');
      pageHeader.style.cssText = 'font-size: 13px; font-weight: 600; color: #eee; display: flex; gap: 8px; align-items: center;';

      if (this.diffMode === 'text') {
        const textData = await this.fetchTextDiff(p);
        if (textData) {
          const hasDiff = textData.additions_count + textData.deletions_count > 0;
          const badgeColor = hasDiff ? '#f87171' : '#34d399';
          const badgeText = hasDiff ? `+${textData.additions_count} / -${textData.deletions_count}` : `✅ Identique`;
          pageHeader.innerHTML = `<span>Page ${p} / ${this.totalPages}</span> <span style="font-size: 11px; background: rgba(0,0,0,0.5); color: ${badgeColor}; padding: 2px 8px; border-radius: 4px;">${badgeText}</span>`;

          const card = document.createElement('div');
          card.style.cssText = 'width: 100%; max-width: 850px; background: var(--bg-primary, #0f172a); border: 1px solid var(--border); border-radius: 8px; padding: 18px; text-align: left; font-family: monospace; font-size: 13px; line-height: 1.7; white-space: pre-wrap;';

          let htmlTokens = '';
          for (const t of textData.tokens) {
            const escaped = t.text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\n/g, '<br/>');
            if (t.op === 'insert') {
              htmlTokens += `<ins style="background: rgba(34, 197, 94, 0.25); color: #4ade80; text-decoration: none; padding: 2px 4px; border-radius: 3px; font-weight: 600;">${escaped}</ins>`;
            } else if (t.op === 'delete') {
              htmlTokens += `<del style="background: rgba(239, 68, 68, 0.25); color: #f87171; text-decoration: line-through; padding: 2px 4px; border-radius: 3px;">${escaped}</del>`;
            } else {
              htmlTokens += `<span style="color: #cbd5e1;">${escaped}</span>`;
            }
          }
          card.innerHTML = htmlTokens || '<em style="color: #64748b;">(Page sans texte)</em>';

          pageWrapper.appendChild(pageHeader);
          pageWrapper.appendChild(card);
        }
      } else {
        const data = await this.fetchPageDiff(p);
        if (data) {
          const pct = (data.diffRatio * 100).toFixed(2);
          const badgeColor = data.hasDiff ? '#f87171' : '#34d399';
          const badgeText = data.hasDiff ? `⚠️ ${pct}% de variation` : `✅ Identique`;
          pageHeader.innerHTML = `<span>Page ${p} / ${this.totalPages}</span> <span style="font-size: 11px; background: rgba(0,0,0,0.5); color: ${badgeColor}; padding: 2px 8px; border-radius: 4px;">${badgeText}</span>`;

          const img = document.createElement('img');
          img.src = data.imgUrl;
          img.style.cssText = 'max-width: 100%; border-radius: 4px; box-shadow: 0 4px 12px rgba(0,0,0,0.5);';
          img.alt = `Diff Page ${p}`;

          pageWrapper.appendChild(pageHeader);
          pageWrapper.appendChild(img);
        }
      }
      frag.appendChild(pageWrapper);
    }

    this.container.innerHTML = '';
    this.container.appendChild(frag);
  }
}
