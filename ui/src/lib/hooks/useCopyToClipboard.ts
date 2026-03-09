import { useState } from "react";

type CopiedValue = string | null;
type CopyFn = (text: string) => Promise<boolean>; // Return success

/**
 * Copies text to clipboard.
 *
 * Returns function to copy, and the copied text (or null if nothing copied).
 */
export function useCopyToClipboard(): [CopyFn, CopiedValue] {
	const [copiedText, setCopiedText] = useState<CopiedValue>(null);

	const copy: CopyFn = async (text) => {
		const copied = await copyToClipboard(text);
		if (copied) setCopiedText(text);
		else setCopiedText(null);
		return copied;
	};

	return [copy, copiedText];
}

/**
 * Copies text to clipboard.
 */
export const copyToClipboard = async (text: string): Promise<boolean> => {
	if (!navigator?.clipboard) {
		logn.error("copyToClipboard", "Clipboard not supported");
		return false;
	}

	const [, error] = await run(async () => {
		await navigator.clipboard.writeText(text);
		return true;
	});

	if (error) {
		logn.error("copyToClipboard", error);
		return false;
	}

	return true;
};
