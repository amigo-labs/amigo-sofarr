// See https://svelte.dev/docs/kit/types#app.d.ts
declare namespace App {
  interface Error {
    message: string;
    /** The error is a script or style of the build that could not be loaded (AC-19). */
    asset?: boolean;
  }
}
