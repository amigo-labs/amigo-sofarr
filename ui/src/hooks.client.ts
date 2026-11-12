import type { ClientInit, HandleClientError } from '@sveltejs/kit/hooks';
import { navigating } from '$app/state';
import { recover } from '#lib/asset-recovery.ts';
import { ASSET_ERROR_MESSAGE, isAssetLoadError } from '#lib/stale-asset.ts';

export const init: ClientInit = () => {
  // Vite reports a failed preload of a lazily loaded module here (spec web-ui-shell AC-19).
  window.addEventListener('vite:preloadError', () => {
    recover(navigating.to?.url ?? new URL(location.href));
  });
};

export const handleError: HandleClientError = (input) => {
  if (input.kind === 'unknown' && isAssetLoadError(input.error)) {
    recover(input.event.url);
    return { message: ASSET_ERROR_MESSAGE, asset: true };
  }
  if (input.kind === 'framework' && input.error.status === 404) {
    return { message: 'Seite nicht gefunden' };
  }
  if (input.kind === 'app') return input.error;
  return { message: 'Unerwarteter Fehler in der Oberfläche.' };
};
