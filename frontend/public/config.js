// Runtime configuration, read by the app before it renders.
//
// This copy is the empty default that ships in the bundle: with no apiBaseUrl
// the app falls back to its build-time default. The Docker image overwrites
// this file on start-up from the API_BASE_URL environment variable, which is
// how one built image can serve any deployment.
window.__LAB_INVENTORY_CONFIG__ = {};
