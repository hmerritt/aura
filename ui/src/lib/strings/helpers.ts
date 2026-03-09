/**
 * Cut string and add an ellipsis when string is too long.
 */
export const enforceLen = (str: string, len: number, ellipsis = true): string => {
	str = str || "";
	if (str.length > len) {
		return str.slice(0, len) + (ellipsis ? "..." : "");
	}
	return str;
};

/**
 * Use char(s) to pad an input string/number to a certain length.
 */
export const padChar = (
	str: string | number,
	size = 5,
	char = " ",
	append = false
): string => {
	str = String(str);
	while (str.length < size) str = append ? str + char : char + str;
	return str;
};

/**
 * A wrapper for `JSON.parse()` to support `undefined` value
 */
export const parseJSON = (value: string | null): any | undefined => {
	try {
		return value === "undefined" ? undefined : JSON.parse(value ?? "");
	} catch {
		logn.warn("parseJSON", value);
		return undefined;
	}
};

/**
 * Returns the plural form of a given word based on a number.
 */
export const pluralize = (n: number, base: string, plural?: string): string => {
	if (n === 1) {
		return base;
	}
	if (plural) {
		return plural;
	}
	return base + "s";
};

/**
 * Returns `s` without the provided leading `prefix` string.
 *
 * If `s` doesn't start with `prefix`, `s` is returned unchanged.
 *
 * Ported from Go-lang `strings.TrimPrefix`
 */
export const trimPrefix = (s: string, prefix: string) => {
	if (s.startsWith(prefix)) return s.slice(prefix.length);
	return s;
};

/**
 * Returns `s` without the provided trailing `suffix` string.
 *
 * If `s` doesn't start with `suffix`, `s` is returned unchanged.
 *
 * Ported from Go-lang `strings.TrimSuffix`
 */
export const trimSuffix = (s: string, suffix: string) => {
	if (s.endsWith(suffix)) return s.slice(0, s.length - suffix.length);
	return s;
};
