// The pages of the UI in navigation order (spec web-ui-shell AC-08), in the sidebar's groups
// with an icon each (spec ui-shell-redesign AC-01, AC-02). A page appears in the navigation
// once it is built (the library pages: spec web-ui-library).

import homeIcon from '@primer/octicons/build/svg/home-16.svg?raw';
import videoIcon from '@primer/octicons/build/svg/video-16.svg?raw';
import seriesIcon from '@primer/octicons/build/svg/device-desktop-16.svg?raw';
import calendarIcon from '@primer/octicons/build/svg/calendar-16.svg?raw';
import inboxIcon from '@primer/octicons/build/svg/inbox-16.svg?raw';
import telescopeIcon from '@primer/octicons/build/svg/telescope-16.svg?raw';
import blockedIcon from '@primer/octicons/build/svg/circle-slash-16.svg?raw';
import commentIcon from '@primer/octicons/build/svg/comment-16.svg?raw';
import databaseIcon from '@primer/octicons/build/svg/database-16.svg?raw';
import serverIcon from '@primer/octicons/build/svg/server-16.svg?raw';
import zapIcon from '@primer/octicons/build/svg/zap-16.svg?raw';

export const NAV_GROUPS = ['Bibliothek', 'Entdecken', 'Verwaltung'] as const;
export type NavGroup = (typeof NAV_GROUPS)[number];

export type NavPage = {
  readonly label: string;
  readonly href: string;
  readonly built: boolean;
  readonly group: NavGroup;
  /** An Octicon as SVG markup. */
  readonly icon: string;
};

export const PAGES: readonly NavPage[] = [
  { label: 'Übersicht', href: '/', built: true, group: 'Bibliothek', icon: homeIcon },
  { label: 'Filme', href: '/filme', built: true, group: 'Bibliothek', icon: videoIcon },
  { label: 'Serien', href: '/serien', built: true, group: 'Bibliothek', icon: seriesIcon },
  { label: 'Kalender', href: '/kalender', built: true, group: 'Bibliothek', icon: calendarIcon },
  { label: 'Fehlend', href: '/fehlend', built: true, group: 'Bibliothek', icon: inboxIcon },
  { label: 'Entdecken', href: '/entdecken', built: true, group: 'Entdecken', icon: telescopeIcon },
  { label: 'Blocklist', href: '/blocklist', built: true, group: 'Verwaltung', icon: blockedIcon },
  { label: 'Untertitel', href: '/untertitel', built: true, group: 'Verwaltung', icon: commentIcon },
  {
    label: 'Neu encodieren',
    href: '/neu-encodieren',
    built: true,
    group: 'Verwaltung',
    icon: zapIcon,
  },
  { label: 'Backups', href: '/backups', built: true, group: 'Verwaltung', icon: databaseIcon },
  { label: 'System', href: '/system', built: true, group: 'Verwaltung', icon: serverIcon },
];

/** The pages the navigation lists. */
export function navPages(pages: readonly NavPage[] = PAGES): readonly NavPage[] {
  return pages.filter((page) => page.built);
}

/** The listed pages by group, in navigation order; groups without a page are left out. */
export function navGroups(
  pages: readonly NavPage[] = PAGES,
): readonly { readonly label: NavGroup; readonly pages: readonly NavPage[] }[] {
  const listed = navPages(pages);
  return NAV_GROUPS.map((label) => ({
    label,
    pages: listed.filter((page) => page.group === label),
  })).filter((group) => group.pages.length > 0);
}

/** Whether `href` is the page shown at `pathname`. */
export function isCurrent(href: string, pathname: string): boolean {
  return href === pathname || (href !== '/' && pathname.startsWith(`${href}/`));
}
