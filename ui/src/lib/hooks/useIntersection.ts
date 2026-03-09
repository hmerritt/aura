import { RefObject, useEffect, useState } from "react";

interface Args extends IntersectionObserverInit {
	freezeOnceVisible?: boolean;
}

/**
 * Detects visibility of a component on the viewport using the IntersectionObserver API.
 *
 * @param elementRef the ref element (from useRef())
 * @param args optionally root, rootMargin and threshold arguments from the native IntersectionObserver API and freezeOnceVisible to only catch the first appearance
 * @returns the full IntersectionObserver's entry object
 */
export function useIntersection(
	elementRef: RefObject<Element>,
	{ threshold = 0, root = null, rootMargin = "0%", freezeOnceVisible = false }: Args
): IntersectionObserverEntry | undefined {
	const [entry, setEntry] = useState<IntersectionObserverEntry>();

	const frozen = entry?.isIntersecting && freezeOnceVisible;

	const updateEntry = ([entry]: IntersectionObserverEntry[]): void => {
		setEntry(entry);
	};

	useEffect(() => {
		const node = elementRef?.current; // DOM Ref
		const hasIOSupport = !!window.IntersectionObserver;

		if (!hasIOSupport || frozen || !node) return;

		const observerParams = { threshold, root, rootMargin };
		const observer = new IntersectionObserver(updateEntry, observerParams);

		observer.observe(node);

		return () => observer.disconnect();
	}, [elementRef, threshold, root, rootMargin, frozen]);

	return entry;
}
