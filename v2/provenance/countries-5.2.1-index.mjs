import { i as currencySchema, n as currencyConversionSchema, r as currencyRatesSchema, t as countrySchema } from "./types-DcB7_XcN.mjs";
//#endregion
//#region src/helpers.ts
const NOT_FOUND_MESSAGE = "Couldn't find any country that matches your query. If you think this is a bug, please report it via GitHub issues: https://github.com/yusifaliyevpro/countries";
/** Wraps any object as a successful result: `{ ...value, success: true, error: undefined }`. */
function ok(value) {
	return {
		...value,
		success: true,
		error: void 0
	};
}
/** The failure branch of a `CountryListResult`. */
function countryListFailure(error) {
	return {
		success: false,
		countries: void 0,
		meta: void 0,
		error
	};
}
/** The failure branch of a `CountryResult`. */
function countryFailure(error) {
	return {
		success: false,
		country: void 0,
		error
	};
}
/**
* Generic failure branch for the currency results: `{ success: false, error }`
* plus each named data key set to `undefined`, so the discriminated union
* narrows correctly on `success` (e.g. `failure(err, "conversions")`).
*/
function failure(error, ...keys) {
	const result = {
		success: false,
		error
	};
	for (const key of keys) result[key] = void 0;
	return result;
}
/**
* Builds an `Error` that carries everything the server reported: a status-aware
* summary, **all** of the API's error messages, and the raw payload + status on
* `error.cause` (typed as {@link ErrorCause}) for programmatic handling.
*/
function errorFromResponse(response, body, rawText) {
	const serverMessages = (body?.errors ?? []).map((e) => e?.message).filter((m) => Boolean(m));
	const detail = serverMessages.length ? serverMessages.join("; ") : rawText.trim().slice(0, 1e3);
	let summary;
	switch (response.status) {
		case 400:
			summary = "bad request — a query parameter failed validation (e.g. empty `q`, or `limit` outside 1–100)";
			break;
		case 401:
			summary = "unauthorized — the `apiKey` is missing, unrecognized, revoked, or expired";
			break;
		case 403:
			summary = "forbidden — the account is frozen (e.g. monthly request limit reached) or a premium field was requested on a Free plan";
			break;
		case 404:
			summary = serverMessages.length ? "not found — unknown endpoint, or the property isn't readable/searchable" : NOT_FOUND_MESSAGE;
			break;
		case 405:
			summary = "method not allowed — REST Countries endpoints are GET-only";
			break;
		case 410:
			summary = "this REST Countries API version is no longer active — update this package";
			break;
		case 429: {
			const retryAfter = response.headers.get("retry-after");
			summary = `rate limit exceeded${retryAfter ? ` — retry after ${retryAfter}s` : " — slow down or upgrade your plan"}`;
			break;
		}
		case 500:
			summary = "REST Countries server error — try again later";
			break;
		default: summary = response.ok ? "unexpected response shape" : "request failed";
	}
	const status = `HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`;
	const message = `REST Countries: ${summary} (${status})${detail && detail !== summary ? ` — ${detail}` : ""}`;
	const cause = {
		status: response.status,
		statusText: response.statusText,
		body: body ?? rawText,
		headers: Object.fromEntries(response.headers)
	};
	return new Error(message, { cause });
}
//#endregion
//#region src/client.ts
function appendFilters(search, filters) {
	const { region, subregion, continent, landlocked, classification, memberships } = filters;
	if (region !== void 0) search.set("region", region);
	if (subregion !== void 0) search.set("subregion", subregion);
	if (continent !== void 0) search.set("continents", continent);
	if (landlocked !== void 0) search.set("landlocked", String(landlocked));
	for (const [key, value] of Object.entries(classification ?? {})) if (value !== void 0) search.set(`classification.${key}`, String(value));
	for (const [key, value] of Object.entries(memberships ?? {})) if (value !== void 0) search.set(`memberships.${key}`, String(value));
}
/**
* A typed client for the REST Countries **v5** API.
*
* Instantiate it once with your API key, then call its methods anywhere — no
* need to thread the key through every call.
*
* @example
* const restCountries = new RestCountries({ apiKey: process.env.REST_COUNTRIES_API_KEY! });
*
* const { success, country, error } = await restCountries.getCountryByCode({ alpha_3: "CAN", fields: ["names"] });
* if (!success) throw error;
* console.log(country.names.common);
*/
var RestCountries = class {
	#apiKey;
	#baseURL;
	#currenciesBaseURL;
	#fetch;
	constructor(config) {
		const apiKey = config?.apiKey?.trim();
		if (!apiKey) throw new Error("RestCountries: `apiKey` is required and cannot be empty. Get one at https://restcountries.com/sign-up");
		this.#apiKey = apiKey;
		this.#baseURL = config.baseURL ?? "https://api.restcountries.com/countries/v5";
		this.#currenciesBaseURL = config.currenciesBaseURL ?? "https://api.restcountries.com/currencies/v1";
		this.#fetch = config.fetch ?? globalThis.fetch;
	}
	/**
	* Performs a request against the v5 API and unwraps the `data` envelope into
	* a {@link CountryListResult}. Never throws — failures are returned as
	* `{ success: false, error }`.
	*/
	async #request(path, params, fetchOptions) {
		try {
			const url = new URL(this.#baseURL.replace(/\/$/, "") + path);
			if (params.q !== void 0) url.searchParams.set("q", params.q);
			if (params.limit !== void 0) url.searchParams.set("limit", String(params.limit));
			if (params.offset !== void 0) url.searchParams.set("offset", String(params.offset));
			if (params.filters) appendFilters(url.searchParams, params.filters);
			const fields = params.fields && Array.from(new Set(params.fields));
			if (fields && fields.length) url.searchParams.set("response_fields", fields.join(","));
			const omit = params.omitFields && Array.from(new Set(params.omitFields));
			if (omit && omit.length) url.searchParams.set("response_fields_omit", omit.join(","));
			const response = await this.#fetch(url.toString(), {
				...fetchOptions,
				headers: {
					Authorization: `Bearer ${this.#apiKey}`,
					...fetchOptions?.headers
				}
			});
			const rawText = await response.text();
			let body;
			try {
				body = rawText ? JSON.parse(rawText) : void 0;
			} catch {
				body = void 0;
			}
			if (!response.ok || !body?.data?.objects) return countryListFailure(errorFromResponse(response, body, rawText));
			const countries = body.data.objects.map(({ _match, _meta, ...country }) => country);
			return ok({
				countries,
				meta: body.data.meta ?? {
					total: countries.length,
					count: countries.length,
					limit: countries.length,
					offset: 0,
					more: false,
					request_id: "",
					duration: 0
				}
			});
		} catch (error) {
			return countryListFailure(error instanceof Error ? error : new Error(String(error)));
		}
	}
	/** Like {@link RestCountries.#request} but resolves to a single-country {@link CountryResult}. */
	async #first(path, params, fetchOptions) {
		const result = await this.#request(path, {
			...params,
			limit: 1
		}, fetchOptions);
		if (!result.success) return countryFailure(result.error);
		const country = result.countries[0];
		if (!country) return countryFailure(/* @__PURE__ */ new Error(NOT_FOUND_MESSAGE));
		return ok({ country });
	}
	/**
	* Fetches countries, optionally narrowed by composable {@link CountryFilters}
	* and/or a free-text `q`, with pagination. With no arguments it lists all
	* countries (paginated).
	*
	* @example
	* // EU countries that are landlocked
	* const { countries } = await restCountries.getCountries({
	*   filters: { region: "Europe", landlocked: true, memberships: { eu: true } },
	*   fields: ["names"],
	* });
	*/
	async getCountries({ q, filters, fields, omitFields, limit, offset } = {}, fetchOptions) {
		return this.#request("", {
			q,
			filters,
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Free-text search across all searchable properties (the v5 `?q=` endpoint).
	* Optionally narrowed by {@link CountryFilters}.
	*
	* @example
	* const { countries } = await restCountries.search("island", { filters: { region: "Oceania" }, fields: ["names"] });
	*/
	async search(q, { filters, fields, omitFields, limit, offset } = {}, fetchOptions) {
		return this.#request("", {
			q,
			filters,
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches a single country by **one** code, identified by its kind. Pass
	* exactly one of `alpha_2`, `alpha_3`, `ccn3`, or `cioc` (see {@link CountryCodeQuery}).
	*
	* @example
	* const { success, country, error } = await restCountries.getCountryByCode({ alpha_3: "USA", fields: ["names", "flag"] });
	* if (success) console.log(country.names.common);
	*
	* @example
	* await restCountries.getCountryByCode({ cioc: "SUI" }); // Switzerland by IOC code
	*/
	async getCountryByCode({ alpha_2, alpha_3, ccn3, cioc, fields, omitFields }, fetchOptions) {
		const match = [
			["codes.alpha_2", alpha_2],
			["codes.alpha_3", alpha_3],
			["codes.ccn3", ccn3],
			["codes.cioc", cioc]
		].find(([, value]) => value !== void 0);
		if (!match) return countryFailure(/* @__PURE__ */ new Error("RestCountries: provide exactly one country code — `alpha_2`, `alpha_3`, `ccn3`, or `cioc`."));
		const [property, value] = match;
		return this.#first(`/${property}/${encodeURIComponent(String(value))}`, {
			fields,
			omitFields
		}, fetchOptions);
	}
	/**
	* Fetches countries matching a name. Searches common, official, alternate and
	* native names. Set `fullText` to require an exact common-name match.
	*
	* @example
	* const { countries } = await restCountries.getCountriesByName({ name: "united", fields: ["names"] });
	* const finland = await restCountries.getCountriesByName({ name: "Finland", fullText: true });
	*/
	async getCountriesByName({ name, fullText, fields, omitFields, limit, offset }, fetchOptions) {
		const path = fullText ? `/names.common/${encodeURIComponent(name)}` : "/name";
		return this.#request(path, {
			q: fullText ? void 0 : name,
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches countries in the given region (e.g. "Europe", "Asia").
	*
	* @example
	* const { countries } = await restCountries.getCountriesByRegion({ region: "Europe", fields: ["names"] });
	*/
	async getCountriesByRegion({ region, fields, omitFields, limit, offset }, fetchOptions) {
		return this.#request(`/region/${encodeURIComponent(region)}`, {
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches countries in the given subregion (e.g. "Northern Europe").
	*
	* @example
	* const { countries } = await restCountries.getCountriesBySubregion({ subregion: "Northern Europe" });
	*/
	async getCountriesBySubregion({ subregion, fields, omitFields, limit, offset }, fetchOptions) {
		return this.#request(`/subregion/${encodeURIComponent(subregion)}`, {
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches countries where the given language is spoken (matches by language name).
	*
	* @example
	* const { countries } = await restCountries.getCountriesByLang({ lang: "Spanish", fields: ["names"] });
	*/
	async getCountriesByLang({ lang, fields, omitFields, limit, offset }, fetchOptions) {
		return this.#request("/languages", {
			q: lang,
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches countries that use the given currency (matches by currency code or name).
	*
	* @example
	* const { countries } = await restCountries.getCountriesByCurrency({ currency: "EUR", fields: ["names"] });
	*/
	async getCountriesByCurrency({ currency, fields, omitFields, limit, offset }, fetchOptions) {
		return this.#request("/currencies", {
			q: currency,
			fields,
			omitFields,
			limit,
			offset
		}, fetchOptions);
	}
	/**
	* Fetches a single country by its capital city.
	*
	* @example
	* const { success, country } = await restCountries.getCountryByCapital({ capital: "Tokyo", fields: ["names"] });
	*/
	async getCountryByCapital({ capital, fields, omitFields }, fetchOptions) {
		return this.#first("/capitals", {
			q: capital,
			fields,
			omitFields
		}, fetchOptions);
	}
	/**
	* Fetches a single country by demonym (e.g. "Canadian", "French").
	*
	* @example
	* const { success, country } = await restCountries.getCountryByDemonym({ demonym: "Canadian", fields: ["names"] });
	*/
	async getCountryByDemonym({ demonym, fields, omitFields }, fetchOptions) {
		return this.#first("/demonyms", {
			q: demonym,
			fields,
			omitFields
		}, fetchOptions);
	}
	/**
	* Fetches a single country by a translated name (e.g. "Deutschland", "Alemania").
	*
	* @example
	* const { success, country } = await restCountries.getCountryByTranslation({ translation: "Deutschland", fields: ["names"] });
	*/
	async getCountryByTranslation({ translation, fields, omitFields }, fetchOptions) {
		return this.#first("/names.translations", {
			q: translation,
			fields,
			omitFields
		}, fetchOptions);
	}
	/**
	* Performs a GET against the Currencies API and unwraps the `data.objects`
	* envelope. Never throws — failures are returned as `{ success: false, error }`.
	*/
	async #currencyRequest(path, search, fetchOptions) {
		try {
			const url = new URL(this.#currenciesBaseURL.replace(/\/$/, "") + path);
			search.forEach((value, key) => url.searchParams.set(key, value));
			const response = await this.#fetch(url.toString(), {
				...fetchOptions,
				headers: {
					Authorization: `Bearer ${this.#apiKey}`,
					...fetchOptions?.headers
				}
			});
			const rawText = await response.text();
			let body;
			try {
				body = rawText ? JSON.parse(rawText) : void 0;
			} catch {
				body = void 0;
			}
			if (!response.ok || !body?.data?.objects) return {
				success: false,
				error: errorFromResponse(response, body, rawText)
			};
			return {
				success: true,
				objects: body.data.objects
			};
		} catch (error) {
			return {
				success: false,
				error: error instanceof Error ? error : new Error(String(error))
			};
		}
	}
	/**
	* Converts an amount from one currency into one or more targets (`amount` defaults to 1, `to` accepts up to 5 codes).
	*
	* @example
	* const { conversions } = await restCountries.convertCurrency({ from: "USD", to: ["EUR", "GBP"], amount: 100 });
	*/
	async convertCurrency({ from, to, amount }, fetchOptions) {
		const search = new URLSearchParams({
			from,
			to: (Array.isArray(to) ? to : [to]).join(",")
		});
		if (amount !== void 0) search.set("amount", String(amount));
		const result = await this.#currencyRequest("/convert", search, fetchOptions);
		if (!result.success) return failure(result.error, "conversions");
		return ok({ conversions: result.objects });
	}
	/**
	* Fetches the full exchange-rate table for a base currency (units per one unit of the base).
	*
	* @example
	* const { rates } = await restCountries.getCurrencyRates({ base: "USD" });
	*/
	async getCurrencyRates({ base }, fetchOptions) {
		const result = await this.#currencyRequest(`/rates/${encodeURIComponent(base)}`, new URLSearchParams(), fetchOptions);
		if (!result.success) return failure(result.error, "base", "as_of", "rates");
		const table = result.objects[0];
		if (!table) return failure(/* @__PURE__ */ new Error(NOT_FOUND_MESSAGE), "base", "as_of", "rates");
		return ok(table);
	}
	/**
	* Fetches the supported-currency catalog (one `{ code, name, symbol }` per currency, sorted by code).
	*
	* @example
	* const { currencies } = await restCountries.getCurrencies();
	*/
	async getCurrencies(fetchOptions) {
		const result = await this.#currencyRequest("/symbols", new URLSearchParams(), fetchOptions);
		if (!result.success) return failure(result.error, "currencies");
		return ok({ currencies: result.objects });
	}
};
//#endregion
//#region src/index.ts
/**
* Type-safe helper for declaring a `fields` array outside of a call site while
* preserving the literal tuple type (so `CountryPicker` can narrow correctly).
*
* @example
* const fields = defineFields(["names", "codes", "capitals"]);
* const country = await restCountries.getCountryByCode({ code: "CAN", fields });
*/
const defineFields = (fields) => fields;
//#endregion
export { RestCountries, countrySchema, currencyConversionSchema, currencyRatesSchema, currencySchema, defineFields, ok };
