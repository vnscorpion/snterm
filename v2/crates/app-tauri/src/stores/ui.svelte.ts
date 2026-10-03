// Hộp thoại dùng chung (message/confirm/input) + menu chuột phải + trạng thái.
export interface MenuItem { label: string; shortcut?: string; disabled?: boolean; danger?: boolean; sep?: boolean; action?: () => void; }

interface MessageBox { kind: 'message' | 'confirm' | 'input'; title: string; text: string; detail?: string; defaultValue?: string; icon?: 'info' | 'warn' | 'error' | 'question';
  okLabel?: string; cancelLabel?: string; resolve: (v: unknown) => void; }

class UiStore {
  boxes = $state<MessageBox[]>([]);
  menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  status = $state('');
  busy = $state(false);

  message(title: string, text: string, icon: MessageBox['icon'] = 'info', detail?: string): Promise<void> {
    return new Promise((resolve) => { this.boxes = [...this.boxes, { kind: 'message', title, text, icon, detail, resolve: () => resolve() }]; });
  }
  confirm(title: string, text: string, icon: MessageBox['icon'] = 'question', okLabel?: string, cancelLabel?: string): Promise<boolean> {
    return new Promise((resolve) => { this.boxes = [...this.boxes, { kind: 'confirm', title, text, icon, okLabel, cancelLabel, resolve: (v) => resolve(!!v) }]; });
  }
  input(title: string, text: string, defaultValue = ''): Promise<string | null> {
    return new Promise((resolve) => { this.boxes = [...this.boxes, { kind: 'input', title, text, defaultValue, resolve: (v) => resolve(v as string | null) }]; });
  }
  close(box: MessageBox, value: unknown) { this.boxes = this.boxes.filter((b) => b !== box); box.resolve(value); }
  openMenu(e: MouseEvent, items: MenuItem[]) { e.preventDefault(); e.stopPropagation(); this.menu = { x: e.clientX, y: e.clientY, items }; }
  closeMenu() { this.menu = null; }
}
export const ui = new UiStore();
