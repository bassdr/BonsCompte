// Forked SVAR components with enhancements:
// - Material: separate `fonts` and `icons` props (icons default true, fonts default false)
// - Text: accepts Svelte component icons (Lucide SVG) instead of CSS class strings
// - DatePicker: `disabledButtons` prop + Lucide Calendar icon via forked Text
// - Calendar: `disabledButtons` prop passed through to Panel
// - calendar/Panel: handles disabledButtons logic
// - calendar/Button: supports disabled state with proper styling

export { default as Material } from './Material.svelte';
export { default as Text } from './Text.svelte';
export { default as DatePicker } from './DatePicker.svelte';
export { default as Calendar } from './Calendar.svelte';

// Re-export commonly used SVAR components that we don't need to fork
export { Locale } from '@svar-ui/svelte-core';
