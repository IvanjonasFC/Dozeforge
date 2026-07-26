// Light / dark theme toggle, persisted in localStorage and applied to <html>.
type Theme = 'dark' | 'light';

const STORAGE_KEY = 'dozeforge-theme';

class ThemeStore {
  theme = $state<Theme>('dark');

  constructor() {
    if (typeof localStorage !== 'undefined') {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved === 'light' || saved === 'dark') this.theme = saved;
    }
  }

  /** Reflect the current theme onto the document root. Call once on mount. */
  apply() {
    if (typeof document !== 'undefined') {
      document.documentElement.setAttribute('data-theme', this.theme);
    }
  }

  toggle() {
    this.theme = this.theme === 'dark' ? 'light' : 'dark';
    if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, this.theme);
    this.apply();
  }
}

export const themeStore = new ThemeStore();
