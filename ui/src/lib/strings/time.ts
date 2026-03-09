const MINUTE = 60;
const HOUR = MINUTE * 60;
const DAY = HOUR * 24;
const MONTH = DAY * 28;
const YEAR = DAY * 365;

/**
 * Time past from a given date or timestamp.
 *
 * After 12 months will return full date as `dd/mm/yyyy`
 */
export const ago = (date: number | string | Date): string => {
	let ts: number;
	if (typeof date === "string") {
		ts = Number(new Date(date));
	} else if (date instanceof Date) {
		ts = Number(date);
	} else {
		ts = date;
	}
	const diffSeconds = Math.floor((Date.now() - ts) / 1e3);
	if (diffSeconds < MINUTE) {
		return `${diffSeconds}s`;
	} else if (diffSeconds < HOUR) {
		return `${Math.floor(diffSeconds / MINUTE)}m`;
	} else if (diffSeconds < DAY) {
		return `${Math.floor(diffSeconds / HOUR)}h`;
	} else if (diffSeconds < MONTH) {
		return `${Math.floor(diffSeconds / DAY)}d`;
	} else if (diffSeconds < YEAR) {
		return `${Math.floor(diffSeconds / MONTH)}mo`;
	} else {
		return new Date(ts).toLocaleDateString();
	}
};

/**
 * Returns date in format `Jul 4, 2023 at 14:03`
 */
export const niceDate = (date: number | string | Date) => {
	const d = new Date(date);
	return `${d.toLocaleDateString("en-us", {
		year: "numeric",
		month: "short",
		day: "numeric"
	})} at ${d.toLocaleTimeString(undefined, {
		hour: "numeric",
		minute: "2-digit"
	})}`;
};
