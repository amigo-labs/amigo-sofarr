// The pages of the UI in navigation order (spec web-ui-shell AC-08). A page appears in the
// navigation once it is built; the library pages follow in spec web-ui-library.

export type NavPage = { readonly label: string; readonly href: string; readonly built: boolean };

export const PAGES: readonly NavPage[] = [
  { label: 'Übersicht', href: '/', built: false },
  { label: 'Filme', href: '/filme', built: false },
  { label: 'Serien', href: '/serien', built: false },
  { label: 'Kalender', href: '/kalender', built: false },
  { label: 'Fehlend', href: '/fehlend', built: false },
  { label: 'Entdecken', href: '/entdecken', built: false },
  { label: 'Blocklist', href: '/blocklist', built: false },
  { label: 'Backups', href: '/backups', built: false },
  { label: 'System', href: '/system', built: true },
];

/** The pages the navigation lists. */
export function navPages(pages: readonly NavPage[] = PAGES): readonly NavPage[] {
  return pages.filter((page) => page.built);
}

/** Until the overview exists, the start page is the system page. */
export const START_PAGE = '/system';

/** Whether `href` is the page shown at `pathname`. */
export function isCurrent(href: string, pathname: string): boolean {
  return href === pathname || (href !== '/' && pathname.startsWith(`${href}/`));
}
