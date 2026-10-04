export interface FormField {
  id: string;
  name: string;
  page_number: number;
  field_type: 'text' | 'checkbox' | 'radio' | 'choice' | 'signature';
  x: number;
  y: number;
  width: number;
  height: number;
  value: string;
  default_value?: string;
  read_only: boolean;
  required: boolean;
  multiline: boolean;
  options?: string[];
}

export interface FormFieldsSummary {
  has_forms: boolean;
  fields_count: number;
  fields: FormField[];
}

export interface FormFillResponse {
  document_id: string;
  updated_fields_count: number;
  message: string;
}

export class FormLayerRenderer {
  private fields: FormField[] = [];
  private fieldValues: Map<string, any> = new Map();
  private onFieldChangeCallback?: (field: FormField, newValue: any) => void;
  private onSignatureClickCallback?: (field: FormField) => void;

  constructor(
    onFieldChange?: (field: FormField, newValue: any) => void,
    onSignatureClick?: (field: FormField) => void
  ) {
    this.onFieldChangeCallback = onFieldChange;
    this.onSignatureClickCallback = onSignatureClick;
  }

  public setFields(fields: FormField[]) {
    this.fields = fields;
    this.fieldValues.clear();
    for (const f of fields) {
      if (f.field_type === 'checkbox') {
        const isChecked = f.value === 'Yes' || f.value === 'true' || f.value === '1';
        this.fieldValues.set(f.name, isChecked ? 'Yes' : 'Off');
      } else {
        this.fieldValues.set(f.name, f.value || '');
      }
    }
  }

  public getFields(): FormField[] {
    return this.fields;
  }

  public getValue(fieldName: string): any {
    return this.fieldValues.get(fieldName);
  }

  public setValue(fieldName: string, value: any) {
    this.fieldValues.set(fieldName, value);
    // Also update any active input DOM element if present
    const inputs = document.querySelectorAll(`[data-field-name="${fieldName}"]`);
    inputs.forEach((input) => {
      if (input instanceof HTMLInputElement) {
        if (input.type === 'checkbox') {
          input.checked = value === 'Yes' || value === true || value === 'true';
        } else if (input.type === 'radio') {
          input.checked = input.value === String(value);
        } else {
          input.value = String(value);
        }
      } else if (input instanceof HTMLTextAreaElement || input instanceof HTMLSelectElement) {
        input.value = String(value);
      }
    });
  }

  public getAllValues(): Record<string, any> {
    const res: Record<string, any> = {};
    for (const [k, v] of this.fieldValues.entries()) {
      res[k] = v;
    }
    return res;
  }

  public render(pageCard: HTMLElement, pageNum: number, zoom: number) {
    let layer = pageCard.querySelector('.form-layer') as HTMLElement;
    if (!layer) {
      layer = document.createElement('div');
      layer.className = 'form-layer';
      pageCard.appendChild(layer);
    }
    layer.innerHTML = '';

    const pageFields = this.fields.filter((f) => f.page_number === pageNum);
    if (pageFields.length === 0) {
      layer.style.display = 'none';
      return;
    }
    layer.style.display = 'block';

    for (const field of pageFields) {
      const fieldEl = this.createFieldElement(field, zoom);
      layer.appendChild(fieldEl);
    }
  }

  private createFieldElement(field: FormField, zoom: number): HTMLElement {
    const container = document.createElement('div');
    container.className = `acroform-field-container acroform-type-${field.field_type}`;
    container.style.position = 'absolute';
    container.style.left = `${field.x * zoom}px`;
    container.style.top = `${field.y * zoom}px`;
    container.style.width = `${Math.max(field.width * zoom, 16)}px`;
    container.style.height = `${Math.max(field.height * zoom, 16)}px`;
    container.style.zIndex = '15';

    const currentValue = this.fieldValues.get(field.name) ?? field.value ?? '';
    const fontSize = Math.max(10, Math.min(Math.round(field.height * zoom * 0.65), 18 * zoom));

    switch (field.field_type) {
      case 'text': {
        if (field.multiline) {
          const textarea = document.createElement('textarea');
          textarea.className = 'acroform-field acroform-textarea';
          textarea.dataset.fieldName = field.name;
          textarea.dataset.fieldId = field.id;
          textarea.value = currentValue;
          textarea.readOnly = field.read_only;
          textarea.required = field.required;
          textarea.style.fontSize = `${fontSize}px`;
          textarea.addEventListener('input', (e) => {
            const val = (e.target as HTMLTextAreaElement).value;
            this.fieldValues.set(field.name, val);
            this.onFieldChangeCallback?.(field, val);
          });
          container.appendChild(textarea);
        } else {
          const input = document.createElement('input');
          input.type = 'text';
          input.className = 'acroform-field acroform-input-text';
          input.dataset.fieldName = field.name;
          input.dataset.fieldId = field.id;
          input.value = currentValue;
          input.readOnly = field.read_only;
          input.required = field.required;
          input.style.fontSize = `${fontSize}px`;
          input.addEventListener('input', (e) => {
            const val = (e.target as HTMLInputElement).value;
            this.fieldValues.set(field.name, val);
            this.onFieldChangeCallback?.(field, val);
          });
          container.appendChild(input);
        }
        break;
      }

      case 'checkbox': {
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.className = 'acroform-field acroform-input-checkbox';
        checkbox.dataset.fieldName = field.name;
        checkbox.dataset.fieldId = field.id;
        checkbox.checked = currentValue === 'Yes' || currentValue === 'true' || currentValue === true;
        checkbox.disabled = field.read_only;
        checkbox.addEventListener('change', (e) => {
          const checked = (e.target as HTMLInputElement).checked;
          const val = checked ? 'Yes' : 'Off';
          this.fieldValues.set(field.name, val);
          this.onFieldChangeCallback?.(field, val);
        });
        container.appendChild(checkbox);
        break;
      }

      case 'radio': {
        const radio = document.createElement('input');
        radio.type = 'radio';
        radio.name = field.name;
        radio.className = 'acroform-field acroform-input-radio';
        radio.dataset.fieldName = field.name;
        radio.dataset.fieldId = field.id;
        radio.value = field.id;
        radio.checked = currentValue === field.id || currentValue === field.value;
        radio.disabled = field.read_only;
        radio.addEventListener('change', () => {
          if (radio.checked) {
            this.fieldValues.set(field.name, radio.value);
            this.onFieldChangeCallback?.(field, radio.value);
          }
        });
        container.appendChild(radio);
        break;
      }

      case 'choice': {
        const select = document.createElement('select');
        select.className = 'acroform-field acroform-select';
        select.dataset.fieldName = field.name;
        select.dataset.fieldId = field.id;
        select.disabled = field.read_only;
        select.style.fontSize = `${fontSize}px`;

        if (field.options && field.options.length > 0) {
          for (const opt of field.options) {
            const optionEl = document.createElement('option');
            optionEl.value = opt;
            optionEl.textContent = opt;
            if (opt === currentValue) {
              optionEl.selected = true;
            }
            select.appendChild(optionEl);
          }
        } else if (currentValue) {
          const optionEl = document.createElement('option');
          optionEl.value = currentValue;
          optionEl.textContent = currentValue;
          optionEl.selected = true;
          select.appendChild(optionEl);
        }

        select.addEventListener('change', (e) => {
          const val = (e.target as HTMLSelectElement).value;
          this.fieldValues.set(field.name, val);
          this.onFieldChangeCallback?.(field, val);
        });
        container.appendChild(select);
        break;
      }

      case 'signature': {
        const sigBadge = document.createElement('div');
        sigBadge.className = 'acroform-field acroform-signature-badge';
        sigBadge.title = `Champ Signature: ${field.name}`;
        sigBadge.innerHTML = `<span style="font-size: ${Math.max(9, fontSize * 0.8)}px; font-weight: 600; display: flex; align-items: center; justify-content: center; height: 100%; gap: 4px; pointer-events: none;">✍️ ${field.name}</span>`;
        sigBadge.addEventListener('click', () => {
          this.onSignatureClickCallback?.(field);
        });
        container.appendChild(sigBadge);
        break;
      }
    }

    return container;
  }
}
