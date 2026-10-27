import * as z from "zod/mini";
//#region src/data/regions.ts
const REGIONS = [
	"Africa",
	"Americas",
	"Antarctic",
	"Asia",
	"Europe",
	"Oceania"
];
//#endregion
//#region src/data/subregions.ts
const SUBREGIONS = [
	"Australia and New Zealand",
	"Caribbean",
	"Central America",
	"Central Asia",
	"Central Europe",
	"Eastern Africa",
	"Eastern Asia",
	"Eastern Europe",
	"Melanesia",
	"Micronesia",
	"Middle Africa",
	"North America",
	"Northern Africa",
	"Northern Europe",
	"Polynesia",
	"South America",
	"South-Eastern Asia",
	"Southeast Europe",
	"Southern Africa",
	"Southern Asia",
	"Southern Europe",
	"Western Africa",
	"Western Asia",
	"Western Europe"
];
//#endregion
//#region src/types/currencies.ts
/**
* Runtime schema for a **currency** from the REST Countries **Currencies** API,
* and the source of truth for the {@link Currency} type. This is separate from a
* country's `currencies` field (`Country["currencies"]`), which never carries a
* `null` symbol.
*/
const currencySchema = z.strictObject({
	code: z.string(),
	name: z.string(),
	/** `null` where the currency has no conventional sign (e.g. gold, `"XAU"`). */
	symbol: z.union([z.string(), z.null()])
});
/** Runtime schema for a single result from the `convert` endpoint. */
const currencyConversionSchema = z.strictObject({
	from: currencySchema,
	to: currencySchema,
	amount: z.number(),
	rate: z.number(),
	result: z.number(),
	as_of: z.number()
});
/** Runtime schema for the exchange-rate table of a base currency (`rates` endpoint). */
const currencyRatesSchema = z.strictObject({
	base: z.string(),
	as_of: z.number(),
	rates: z.record(z.string(), z.number())
});
//#endregion
//#region src/types/index.ts
const localizedName = z.strictObject({
	common: z.string(),
	official: z.string()
});
const coordinates = z.strictObject({
	lat: z.number(),
	lng: z.number()
});
const dayMonth = z.strictObject({
	day: z.number(),
	month: z.number()
});
/**
* Runtime schema for a country as returned by the REST Countries **v5** API,
* and the single source of truth for the {@link Country} type (inferred below).
*
* It uses {@link https://zod.dev/packages/mini | Zod Mini} for a small footprint
* and `strictObject` everywhere, so {@link countrySchema}`.parse()` rejects any
* property the API adds that isn't modelled here — surfacing schema drift.
*/
const countrySchema = z.strictObject({
	names: z.strictObject({
		common: z.string(),
		official: z.string(),
		alternates: z.array(z.string()),
		native: z.record(z.string(), localizedName),
		translations: z.record(z.string(), localizedName)
	}),
	codes: z.strictObject({
		alpha_2: z.string(),
		alpha_3: z.string(),
		ccn3: z.string(),
		cioc: z.string(),
		fifa: z.string(),
		fips: z.string(),
		gec: z.string()
	}),
	capitals: z.array(z.strictObject({
		name: z.string(),
		coordinates,
		attributes: z.strictObject({
			administrative: z.boolean(),
			constitutional: z.boolean(),
			executive: z.boolean(),
			judicial: z.boolean(),
			legislative: z.boolean(),
			primary: z.boolean()
		})
	})),
	flag: z.strictObject({
		colors: z.strictObject({
			/** Dominant flag color as a hex string. `""` for countries without a flag. */
			dominant: z.string(),
			/** Most prominent flag color (largest share of the flag's area) as a hex string. */
			prominent: z.string(),
			/** Palette of `{ hex, proportion }`, pairing each color with its 0–1 area share. `[]` for countries without a flag. */
			palette: z.array(z.strictObject({
				hex: z.string(),
				proportion: z.number()
			})),
			/** Semantic color swatches by role; all six are always present, `null` when the flag has no matching color. */
			swatches: z.strictObject({
				vibrant: z.union([z.string(), z.null()]),
				muted: z.union([z.string(), z.null()]),
				dark_vibrant: z.union([z.string(), z.null()]),
				dark_muted: z.union([z.string(), z.null()]),
				light_vibrant: z.union([z.string(), z.null()]),
				light_muted: z.union([z.string(), z.null()])
			})
		}),
		description: z.string(),
		emoji: z.string(),
		html_entity: z.string(),
		unicode: z.string(),
		url_png: z.string(),
		url_svg: z.string()
	}),
	region: z.literal(REGIONS),
	subregion: z.literal([...SUBREGIONS, ""]),
	area: z.strictObject({
		kilometers: z.number(),
		miles: z.number()
	}),
	assets: z.array(z.string()),
	borders: z.array(z.string()),
	calling_codes: z.array(z.string()),
	cars: z.strictObject({
		driving_side: z.literal(["left", "right"]),
		signs: z.array(z.string())
	}),
	classification: z.strictObject({
		dependency: z.boolean(),
		dependency_type: z.string(),
		disputed: z.boolean(),
		iso_status: z.string(),
		sovereign: z.boolean(),
		un_member: z.boolean(),
		un_observer: z.boolean()
	}),
	continents: z.array(z.string()),
	coordinates,
	currencies: z.array(z.strictObject({
		code: z.string(),
		name: z.string(),
		symbol: z.string()
	})),
	date: z.strictObject({
		academic_year_start: dayMonth,
		fiscal_year_start: z.strictObject({
			corporate: z.strictObject({
				basis: z.string(),
				day: z.number(),
				month: z.number()
			}),
			government: dayMonth,
			personal: dayMonth
		}),
		start_of_week: z.string()
	}),
	demonyms: z.record(z.string(), z.strictObject({
		f: z.string(),
		m: z.string()
	})),
	economy: z.strictObject({ gini_coefficient: z.record(z.string(), z.number()) }),
	government_type: z.string(),
	landlocked: z.boolean(),
	languages: z.array(z.strictObject({
		bcp47: z.string(),
		iso639_1: z.string(),
		iso639_2b: z.string(),
		iso639_2t: z.string(),
		iso639_3: z.string(),
		name: z.string(),
		native_name: z.string()
	})),
	/** Only populated on paid REST Countries plans; otherwise an upgrade notice. */
	leaders: z.union([z.array(z.strictObject({
		message: z.string(),
		sample: z.string()
	})), z.array(z.strictObject({
		uuid: z.union([z.string(), z.null()]),
		name: z.string(),
		title: z.string(),
		links: z.strictObject({ wikipedia: z.union([z.string(), z.null()]) }),
		attributes: z.strictObject({
			administers_executive: z.union([z.boolean(), z.null()]),
			de_facto_executive: z.union([z.boolean(), z.null()]),
			head_of_government: z.union([z.boolean(), z.null()]),
			head_of_state: z.union([z.boolean(), z.null()]),
			is_representative: z.union([z.boolean(), z.null()]),
			pending_office: z.union([z.boolean(), z.null()]),
			provisional: z.union([z.boolean(), z.null()])
		}),
		assets: z.strictObject({ "wikipedia.article.open_graph_image": z.optional(z.strictObject({
			renditions: z.array(z.strictObject({
				content_hash: z.string(),
				content_type: z.string(),
				filesize: z.number(),
				height: z.number(),
				max_edge: z.number(),
				url: z.string(),
				width: z.number()
			})),
			source: z.string(),
			type: z.string()
		})) })
	}))]),
	links: z.strictObject({
		google_maps: z.string(),
		official: z.string(),
		open_street_maps: z.string(),
		wikipedia: z.string()
	}),
	memberships: z.strictObject({
		african_union: z.boolean(),
		arab_league: z.boolean(),
		asean: z.boolean(),
		brics: z.boolean(),
		commonwealth: z.boolean(),
		eu: z.boolean(),
		eurozone: z.boolean(),
		g20: z.boolean(),
		g7: z.boolean(),
		nato: z.boolean(),
		oecd: z.boolean(),
		opec: z.boolean(),
		schengen: z.boolean(),
		un: z.boolean()
	}),
	units: z.strictObject({
		measurement_system: z.literal(["metric", "imperial"]),
		temperature_scale: z.literal(["Celsius", "Fahrenheit"])
	}),
	number_format: z.strictObject({
		decimal_separator: z.string(),
		thousands_separator: z.string()
	}),
	parent: z.strictObject({
		alpha_2: z.string(),
		alpha_3: z.string()
	}),
	population: z.number(),
	postal_code: z.strictObject({
		format: z.string(),
		regex: z.string()
	}),
	timezones: z.array(z.string()),
	tlds: z.array(z.string()),
	uuid: z.string()
});
//#endregion
export { currencySchema as i, currencyConversionSchema as n, currencyRatesSchema as r, countrySchema as t };
