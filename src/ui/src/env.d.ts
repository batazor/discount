/// <reference types="astro/client" />
declare namespace App {
  interface Locals {
    /** The signed-in operator's claims, or null when auth is off or nobody is signed in. */
    user: import("./auth/oidc").Claims | null;
  }
}
