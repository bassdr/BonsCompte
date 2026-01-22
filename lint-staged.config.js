// Helper to convert absolute paths to relative paths and quote them for shell safety
const toRelative = (basePath, filenames) =>
  filenames.map((f) => `"${f.replace(new RegExp(`^.*/` + basePath + `/`), '')}"`).join(' ');

export default {
  'backend/**/*.rs': (filenames) => {
    // Use cargo fmt to respect edition from Cargo.toml
    const files = toRelative('backend', filenames);
    return `env -C backend cargo fmt -- ${files}`;
  },
  'frontend/**/*.{js,ts,svelte}': (filenames) => {
    // Use relative paths from frontend directory
    const files = toRelative('frontend', filenames);
    return [
      `env -C frontend npx prettier --write ${files}`,
      `env -C frontend npx eslint --fix ${files}`
    ];
  },
  'frontend/**/*.{json,css,md}': (filenames) => {
    const files = toRelative('frontend', filenames);
    return `env -C frontend npx prettier --write ${files}`;
  }
};
