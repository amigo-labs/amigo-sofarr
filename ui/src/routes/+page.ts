import { redirect } from '@sveltejs/kit';
import { START_PAGE } from '#lib/nav.ts';
import type { PageLoad } from './$types';

// Until the overview exists (spec web-ui-library) the start page is the system page (AC-08).
export const load: PageLoad = () => {
  redirect(307, START_PAGE);
};
