/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Backend API the built bundle defaults to.
   *
   * Baked in at build time. Deployments that serve one image to many sites
   * leave it unset and let `config.js` supply the value at run time instead.
   */
  readonly VITE_DEFAULT_API_BASE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
