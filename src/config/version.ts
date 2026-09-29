/**
 * Central application version (Single Source of Truth for the entire UI).
 * Loaded automatically from package.json via Vite compile-time definition (__APP_VERSION__).
 * Falls back to the default below if __APP_VERSION__ is undefined.
 */
export const APP_VERSION = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '1.1.0';

export const APP_VERSION_TAG = `v${APP_VERSION}`;
export const APP_ENGINE_TAG = `${APP_VERSION_TAG} Engine`;
export const APP_ORCHESTRATOR_TAG = `${APP_VERSION_TAG} Orchestrator`;

