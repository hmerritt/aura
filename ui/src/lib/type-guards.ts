/**
 * Conditionally spread an item into an array.
 */
export const arraySpread = <TItem>(
	/** Conditional value. When truthy, `itemToSpread` is returned. */
	conditional: any,
	/** Item that gets returned when conditional is truthy */
	itemToSpread: TItem | TItem[],
	/** Option to spread multiple values when `itemToSpread` is an array */
	spreadMultiple = false
) => {
	if (isTruthy(conditional))
		return spreadMultiple && Array.isArray(itemToSpread)
			? (itemToSpread as TItem[])
			: [itemToSpread as TItem];
	return [] as TItem[];
};

/**
 * Gets the property value at path of object.
 *
 * If the resolved value is undefinedthe defaultValue is used in its place.
 */
export const get = <T = any>(
	object: Record<string, any> | null | undefined,
	path: string | string[],
	defaultValue?: T
): T | undefined => {
	// Coerce path to an array if it's a string.
	const pathArray = Array.isArray(path) ? path : path.split(".");

	// Reduce the path array to the final value.
	let result: any = object;
	for (const key of pathArray) {
		// If at any point the object/value is null or undefined, the path is invalid.
		if (result === null || result === undefined) {
			return defaultValue;
		}
		result = result[key];
	}

	return result === undefined ? defaultValue : result;
};

export const hasProp = <K extends PropertyKey>(
	data: object,
	prop: K
): data is Record<K, unknown> => {
	return prop in data;
};

export const isObj = (v: unknown): v is Record<string, unknown> => {
	return !!v && typeof v === "object" && !Array.isArray(v);
};

/**
 * @returns `true` when all supplied values are NOT any of the bellow:
 * - Is falsy
 * - Is empty array
 * - Is empty object
 */
export const isTruthy = (...v: any[]) => {
	for (let i = 0; i < v.length; i++) {
		if (
			!v[i] ||
			(Array.isArray(v[i]) && v[i]?.length === 0) ||
			(isObj(v[i]) && Object.keys(v[i] ?? {})?.length === 0)
		)
			return false;
	}
	return true;
};

/**
 * Type-safe version of `Object.keys`
 */
export const objKeys = Object.keys as <T extends object>(obj: T) => Array<keyof T>;
