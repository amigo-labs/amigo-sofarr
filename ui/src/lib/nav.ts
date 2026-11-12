// The pages of the UI in navigation order (spec web-ui-shell AC-08). A page appears in the
// navigation once it is built (the library pages: spec web-ui-library).

export type NavPage = { readonly label: string; readonly href: string; readonly built: boolean };

export const PAGES: readonly NavPage[] = [
  { label: 'Übersicht', href: '/', built: true },
  { label: 'Filme', href: '/filme', built: true },
  { label: 'Serien', href: '/serien', built: true },
  { label: 'Kalender', href: '/kalender', built: true },
  { label: 'Fehlend', href: '/fehlend', built: true },
  { label: 'Entdecken', href: '/entdecken', built: false },
  { label: 'Blocklist', href: '/blocklist', built: true },
  { label: 'Untertitel', href: '/untertitel', built: true },
  { label: 'Backups', href: '/backups', built: true },
  { label: 'System', href: '/system', built: true },
];

/** The pages the navigation lists. */
export function navPages(pages: readonly NavPage[] = PAGES): readonly NavPage[] {
  return pages.filter((page) => page.built);
}

/** Whether `href` is the page shown at `pathname`. */
export function isCurrent(href: string, pathname: string): boolean {
  return href === pathname || (href !== '/' && pathname.startsWith(`${href}/`));
}
