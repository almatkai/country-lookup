/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** API Key - Your REST Countries API key. Get one for free at https://restcountries.com/sign-up. */
  "apiKey": string,
  /** Show Countries - REST Countries lists a few partially-recognized territories that have no ISO 3166-1 code (for example Abkhazia, South Ossetia and Northern Cyprus). Choose whether to include them. This is based purely on the data source's ISO codes and is not a political stance by this extension or its author. */
  "entriesWithoutIsoCode": "show" | "hide"
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `index` command */
  export type Index = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `index` command */
  export type Index = {}
}

