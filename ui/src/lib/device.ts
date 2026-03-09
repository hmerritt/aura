/////////////////////////////////////////////////////////////////////////////////
/* Device detection.
   Heavily modified version originally ported from UAParser.js v2.0.5
   Source : https://github.com/faisalman/ua-parser-js */
/////////////////////////////////////////////////////////////////////////////////

// region Types

/**
 * Represents the type of the device being identified.
 */
type DeviceType = "mobile" | "tablet";

/**
 * Defines the shape of an object that holds device properties.
 * The `type` property is optional as it's determined during parsing.
 */
interface IDevice {
	[key: string]: any;
	type?: DeviceType;
}

// Using a type alias for the complex array structure in the regex map for clarity.
type RegexMap = Array<Array<RegExp> | Array<Array<string | Function | RegExp>>>;

// region Constants

const TYPE: "type" = "type";
const MOBILE: DeviceType = "mobile";
const TABLET: DeviceType = "tablet";

// A safely accessed window.navigator.userAgent.
const navUserAgent =
	typeof window !== "undefined" && window.navigator ? window.navigator.userAgent : "";

//
// Regular Expression Map for Device Identification
// A curated list of regular expressions to identify various mobile and tablet devices.
//

const deviceRegexMap: RegexMap = [
	[
		// Samsung
		/\b(sch-i[89]0\d|shw-m380s|sm-[ptx]\w{2,4}|gt-[pn]\d{2,4}|sgh-t8[56]9|nexus 10)/i
	],
	[[TYPE, TABLET]],
	[
		/\b((?:s[cgp]h|gt|sm)-(?![lr])\w+|sc[g-]?[\d]+a?|galaxy nexus)/i,
		/samsung[- ]((?!sm-[lr]|browser)[-\w]+)/i,
		/sec-(sgh\w+)/i
	],
	[[TYPE, MOBILE]],
	[
		// Apple
		/(?:\/|\()(ip(?:hone|od)[\w, ]*)[\/\);]/i // iPod/iPhone
	],
	[[TYPE, MOBILE]],
	[
		/\b(?:ios|apple\w+)\/.+[\(\/](ipad)/i, // iPad
		/\b(ipad)[\d,]*[;\] ].+(mac |i(pad)?)os/i
	],
	[[TYPE, TABLET]],
	[
		// Sharp
		/\b(sh-?[altvz]?\d\d[a-ekm]?)/i
	],
	[[TYPE, MOBILE]],
	[
		// Honor
		/\b((?:brt|eln|hey2?|gdi|jdn)-a?[lnw]09|(?:ag[rm]3?|jdn2|kob2)-a?[lw]0[09]hn)(?: bui|\)|;)/i
	],
	[[TYPE, TABLET]],
	[/honor([-\w ]+)[;\)]/i],
	[[TYPE, MOBILE]],
	[
		// Huawei
		/\b((?:ag[rs][2356]?k?|bah[234]?|bg[2o]|bt[kv]|cmr|cpn|db[ry]2?|jdn2|got|kob2?k?|mon|pce|scm|sht?|[tw]gr|vrd)-[ad]?[lw][0125][09]b?|605hw|bg2-u03|(?:gem|fdr|m2|ple|t1)-[7a]0[1-4][lu]|t1-a2[13][lw]|mediapad[\w\. ]*(?= bui|\)))\b(?!.+d\/s)/i
	],
	[[TYPE, TABLET]],
	[
		/(?:huawei)([-\w ]+)[;\)]/i,
		/\b(nexus 6p|\w{2,4}e?-[atu]?[ln][\dx][012359c][adn]?)\b(?!.+d\/s)/i
	],
	[[TYPE, MOBILE]],
	[
		// Xiaomi
		/oid[^\)]+; (2[\dbc]{4}(182|283|rp\w{2})[cgl]|m2105k81a?c)(?: bui|\))/i,
		/\b((?:red)?mi[-_ ]?pad[\w- ]*)(?: bui|\))/i // Mi Pad tablets
	],
	[
		[/_/g, " "],
		[TYPE, TABLET]
	],
	[
		/\b(poco[\w ]+|m2\d{3}j\d\d[a-z]{2})(?: bui|\))/i, // Xiaomi POCO
		/\b; (\w+) build\/hm\1/i, // Xiaomi Hongmi 'numeric' models
		/\b(hm[-_ ]?note?[_ ]?(?:\d\w)?) bui/i, // Xiaomi Hongmi
		/\b(redmi[\-_ ]?(?:note|k)?[\w_ ]+)(?: bui|\))/i, // Xiaomi Redmi
		/oid[^\)]+; (m?[12][0-389][01]\w{3,6}[c-y])( bui|; wv|\))/i, // Xiaomi Redmi 'numeric' models
		/\b(mi[-_ ]?(?:a\d|one|one[_ ]plus|note lte|max|cc)?[_ ]?(?:\d?\w?)[_ ]?(?:plus|se|lite|pro)?)(?: bui|\))/i, // Xiaomi Mi
		/ ([\w ]+) miui\/v?\d/i
	],
	[
		[/_/g, " "],
		[TYPE, MOBILE]
	],
	[
		// OnePlus
		/droid.+; (cph2[3-6]\d[13579]|((gm|hd)19|(ac|be|in|kb)20|(d[en]|eb|le|mt)21|ne22)[0-2]\d|p[g-k]\w[1m]10)\b/i,
		/(?:one)?(?:plus)? (a\d0\d\d)(?: b|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// OPPO
		/; (\w+) bui.+ oppo/i,
		/\b(cph[12]\d{3}|p(?:af|c[al]|d\w|e[ar])[mt]\d0|x9007|a101op)\b/i
	],
	[[TYPE, MOBILE]],
	[/\b(opd2(\d{3}a?))(?: bui|\))/i],
	[[TYPE, TABLET]],
	[
		// BLU
		/(vivo (5r?|6|8l?|go|one|s|x[il]?[2-4]?)[\w\+ ]*)(?: bui|\))/i // Vivo series
	],
	[[TYPE, MOBILE]],
	[
		// Vivo
		/; vivo (\w+)(?: bui|\))/i,
		/\b(v[12]\d{3}\w?[at])(?: bui|;)/i
	],
	[[TYPE, MOBILE]],
	[
		// Realme
		/\b(rmx[1-3]\d{3})(?: bui|;|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// Lenovo
		/(ideatab[-\w ]+|602lv|d-42a|a101lv|a2109a|a3500-hv|s[56]000|pb-6505[my]|tb-?x?\d{3,4}(?:f[cu]|xu|[av])|yt\d?-[jx]?\d+[lfmx])( bui|;|\)|\/)/i,
		/lenovo ?(b[68]0[08]0-?[hf]?|tab(?:[\w- ]+?)|tb[\w-]{6,7})( bui|;|\)|\/)/i
	],
	[[TYPE, TABLET]],
	[/lenovo[-_ ]?([-\w ]+?)(?: bui|\)|\/)/i],
	[[TYPE, MOBILE]],
	[
		// Motorola
		/\b(milestone|droid(?:[2-4x]| (?:bionic|x2|pro|razr))?:?( 4g)?)\b[\w ]+build\//i,
		/\bmot(?:orola)?[- ]([\w\s]+)(\)| bui)/i,
		/((?:moto(?! 360)[-\w\(\) ]+|xt\d{3,4}[cgkosw\+]?[-\d]*|nexus 6)(?= bui|\)))/i
	],
	[[TYPE, MOBILE]],
	[/\b(mz60\d|xoom[2 ]{0,2}) build\//i],
	[[TYPE, TABLET]],
	[
		// LG
		/((?=lg)?[vl]k\-?\d{3}) bui| 3\.[-\w; ]{10}lg?-([06cv9]{3,4})/i
	],
	[[TYPE, TABLET]],
	[
		/(lm(?:-?f100[nv]?|-[\w\.]+)(?= bui|\))|nexus [45])/i,
		/\blg[-e;\/ ]+(?!.*(?:browser|netcast|android tv|watch|webos))(\w+)/i,
		/\blg-?([\d\w]+) bui/i
	],
	[[TYPE, MOBILE]],
	[
		// Nokia
		/(nokia) (t[12][01])/i
	],
	[[TYPE, TABLET]],
	[/(?:maemo|nokia).*(n900|lumia \d+|rm-\d+)/i, /nokia[-_ ]?(([-\w\. ]*))/i],
	[
		[/_/g, " "],
		[TYPE, MOBILE]
	],
	[
		// Google
		/(pixel (c|tablet))\b/i // Google Pixel C/Tablet
	],
	[[TYPE, TABLET]],
	[
		// Google Pixel
		/droid.+;(?: google)? (g(01[13]a|020[aem]|025[jn]|1b60|1f8f|2ybb|4s1m|576d|5nz6|8hhn|8vou|a02099|c15s|d1yq|e2ae|ec77|gh2x|kv4x|p4bc|pj41|r83y|tt9q|ur25|wvk6)|pixel[\d ]*a?( pro)?( xl)?( fold)?( \(5g\))?)( bui|\))/i
	],
	[[TYPE, MOBILE]],
	[/(google) (pixelbook( go)?)/i],
	[],
	[
		// Sony
		/droid.+; (a?\d[0-2]{2}so|[c-g]\d{4}|so[-gl]\w+|xq-\w\w\d\d)(?= bui|\).+chrome\/(?![1-6]{0,1}\d\.))/i
	],
	[[TYPE, MOBILE]],
	[/sony tablet [ps]/i, /\b(?:sony)?sgp\w+(?: bui|\))/i],
	[[TYPE, TABLET]],
	[
		// Amazon
		/(alexa)webm/i,
		/(kf[a-z]{2}wi|aeo(?!bc)\w\w)( bui|\))/i, // Kindle Fire without Silk / Echo Show
		/(kf[a-z]+)( bui|\)).+silk\//i // Kindle Fire HD
	],
	[[TYPE, TABLET]],
	[
		/((?:sd|kf)[0349hijorstuw]+)( bui|\)).+silk\//i // Fire Phone
	],
	[[TYPE, MOBILE]],
	[
		// BlackBerry
		/(playbook);[-\w\),; ]+(rim)/i // BlackBerry PlayBook
	],
	[[TYPE, TABLET]],
	[
		/\b((?:bb[a-f]|st[hv])100-\d)/i,
		/\(bb10; (\w+)/i // BlackBerry 10
	],
	[[TYPE, MOBILE]],
	[
		// Asus
		/(?:\b|asus_)(transfo[prime ]{4,10} \w+|eeepc|slider \w+|nexus 7|padfone|p00[cj])/i
	],
	[[TYPE, TABLET]],
	[/ (z[bes]6[027][012][km][ls]|zenfone \d\w?)\b/i],
	[[TYPE, MOBILE]],
	[
		// HTC
		/(nexus 9)/i // HTC Nexus 9
	],
	[[TYPE, TABLET]],
	[
		/(htc)[-;_ ]{1,2}([\w ]+(?=\)| bui)|\w+)/i, // HTC

		// ZTE
		/(zte)[- ]([\w ]+?)(?: bui|\/|\))/i,
		/(alcatel|geeksphone|nexian|panasonic(?!(?:;|\.))|sony(?!-bra))[-_ ]?([-\w]*)/i // Alcatel/GeeksPhone/Nexian/Panasonic/Sony
	],
	[[TYPE, MOBILE]],
	[
		// TCL
		/tcl (xess p17aa)/i,
		/droid [\w\.]+; ((?:8[14]9[16]|9(?:0(?:48|60|8[01])|1(?:3[27]|66)|2(?:6[69]|9[56])|466))[gqswx])(_\w(\w|\w\w))?(\)| bui)/i
	],
	[[TYPE, TABLET]],
	[
		/droid [\w\.]+; (418(?:7d|8v)|5087z|5102l|61(?:02[dh]|25[adfh]|27[ai]|56[dh]|59k|65[ah])|a509dl|t(?:43(?:0w|1[adepqu])|50(?:6d|7[adju])|6(?:09dl|10k|12b|71[efho]|76[hjk])|7(?:66[ahju]|67[hw]|7[045][bh]|71[hk]|73o|76[ho]|79w|81[hks]?|82h|90[bhsy]|99b)|810[hs]))(_\w(\w|\w\w))?(\)| bui)/i
	],
	[[TYPE, MOBILE]],
	[
		// itel
		/(itel) ((\w+))/i
	],
	[[TYPE, MOBILE]],
	[
		// Acer
		/droid.+; ([ab][1-7]-?[0178a]\d\d?)/i
	],
	[[TYPE, TABLET]],
	[
		// Meizu
		/droid.+; (m[1-5] note) bui/i,
		/\bmz-([-\w]{2,})/i
	],
	[[TYPE, MOBILE]],
	[
		// Ulefone
		/; ((?:power )?armor(?:[\w ]{0,8}))(?: bui|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// Energizer
		/; (energy ?\w+)(?: bui|\))/i,
		/; energizer ([\w ]+)(?: bui|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// Cat
		/; cat (b35);/i,
		/; (b15q?|s22 flip|s48c|s62 pro)(?: bui|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// Smartfren
		/((?:new )?andromax[\w- ]+)(?: bui|\))/i
	],
	[[TYPE, MOBILE]],
	[
		// Nothing
		/droid.+; (a(in)?(0(15|59|6[35])|142)p?)/i
	],
	[[TYPE, MOBILE]],
	[
		// Archos
		/; (x67 5g|tikeasy \w+|ac[1789]\d\w+)( b|\))/i,
		/archos ?(5|gamepad2?|([\w ]*[t1789]|hello) ?\d+[\w ]*)( b|\))/i
	],
	[[TYPE, TABLET]],
	[/archos ([\w ]+)( b|\))/i, /; (ac[3-6]\d\w{2,8})( b|\))/i],
	[[TYPE, MOBILE]],
	[
		// HMD
		/; (n159v)/i
	],
	[[TYPE, MOBILE]],
	[
		// MIXED
		/(imo) (tab \w+)/i, // IMO
		/(infinix|tecno) (x1101b?|p904|dp(7c|8d|10a)( pro)?|p70[1-3]a?|p904|t1101)/i // Infinix XPad / Tecno
	],
	[[TYPE, TABLET]],
	[
		/(blackberry|benq|palm(?=\-)|sonyericsson|acer|asus(?! zenw)|dell|jolla|meizu|motorola|polytron|tecno|micromax|advan)[-_ ]?([-\w]*)/i,
		// BlackBerry/BenQ/Palm/Sony-Ericsson/Acer/Asus/Dell/Meizu/Motorola/Polytron/Tecno/Micromax/Advan
		/; (blu|hmd|imo|infinix|lava|oneplus|tcl)[_ ]([\w\+ ]+?)(?: bui|\)|; r)/i, // BLU/HMD/IMO/Infinix/Lava/OnePlus/TCL
		/(hp) ([\w ]+\w)/i, // HP iPAQ
		/(microsoft); (lumia[\w ]+)/i, // Microsoft Lumia
		/(oppo) ?([\w ]+) bui/i, // OPPO
		/droid[^;]+; (philips)[_ ]([sv-x][\d]{3,4}[xz]?)/i // Philips
	],
	[[TYPE, MOBILE]],
	[
		/(kobo)\s(ereader|touch)/i, // Kobo
		/(hp).+(touchpad(?!.+tablet)|tablet)/i, // HP TouchPad
		/(kindle)\/([\w\.]+)/i // Kindle
	],
	[[TYPE, TABLET]],
	[
		/(surface duo)/i // Surface Duo
	],
	[[TYPE, TABLET]],
	[
		/droid [\d\.]+; (fp\du?)(?: b|\))/i // Fairphone
	],
	[[TYPE, MOBILE]],
	[
		/((?:tegranote|shield t(?!.+d tv))[\w- ]*?)(?: b|\))/i // Nvidia Tablets
	],
	[[TYPE, TABLET]],
	[
		/(sprint) (\w+)/i // Sprint Phones
	],
	[[TYPE, MOBILE]],
	[
		/(kin\.[onetw]{3})/i // Microsoft Kin
	],
	[[TYPE, MOBILE]],
	[
		/droid.+; ([c6]+|et5[16]|mc[239][23]x?|vc8[03]x?)\)/i // Zebra
	],
	[[TYPE, TABLET]],
	[/droid.+; (ec30|ps20|tc[2-8]\d[kx])\)/i],
	[[TYPE, MOBILE]],
	[
		/\b((tablet|tab)[;\/]|focus\/\d(?!.+mobile))/i // Unidentifiable Tablet
	],
	[[TYPE, TABLET]],
	[
		/(phone|mobile(?:[;\/]| [ \w\/\.]*safari)|pda(?=.+windows ce))/i // Unidentifiable Mobile
	],
	[[TYPE, MOBILE]],
	[
		/droid .+?; ([\w\. -]+)( bui|\))/i // Generic Android Device
	]
];

// region Core Logic

/**
 * Maps properties to a device object based on regex matches from the user agent string.
 */
const rgxMapper = (ua: string, arrays: RegexMap): IDevice => {
	const device: IDevice = {};
	if (!ua || !arrays) return device;

	for (let i = 0; i < arrays.length; i += 2) {
		const regexes = arrays[i] as RegExp[];
		const props = arrays[i + 1] as any[];

		for (let j = 0; j < regexes.length; j++) {
			const matches = regexes[j].exec(ua);
			if (matches) {
				for (let k = 0, qPos = 0; k < props.length; k++) {
					const prop = props[k];
					const match = matches[++qPos];
					if (Array.isArray(prop) && prop.length > 0) {
						if (prop.length === 2) {
							device[prop[0]] =
								typeof prop[1] === "function" ? prop[1](match) : prop[1];
						}
					} else {
						device[prop] = match ? match : undefined;
					}
				}
				// Return the device object as soon as a match is found
				// and its type is determined.
				if (device.type) {
					return device;
				}
			}
		}
	}
	return device;
};

/**
 * Parses a user agent string to determine if it belongs to a mobile or tablet device.
 */
export const parseUserAgent = (ua: string = navUserAgent) => {
	const device = rgxMapper(ua, deviceRegexMap);

	return {
		ua,
		isMobile: device.type === MOBILE,
		isTablet: device.type === TABLET
	};
};

// region Exports

const result = parseUserAgent();

/**
 * The user agent string of the current environment.
 */
export const userAgent: string = result.ua;

/**
 * `true` when ran on a mobile device.
 *
 * Uses the `user agent` for accurate detection.
 */
export const isMobile: boolean = result.isMobile;

/**
 * `true` when ran on a tablet device.
 *
 * Uses the `user agent` for accurate detection.
 */
export const isTablet: boolean = result.isTablet;
