import { useEffect, useRef, useState } from "react";

export function useInterval(
	callback: () => void,
	delay: number | null,
	startImmediately = true
) {
	const [started, setStarted] = useState(startImmediately);
	const savedCallback = useRef(callback);

	// Remember the latest callback if it changes.
	useEffect(() => {
		savedCallback.current = callback;
	}, [callback]);

	// Set up the interval.
	useEffect(() => {
		// Skip if not started / is stopped.
		if (!started) return;
		// Don't schedule if no delay is specified.
		// Note: 0 is a valid value for delay.
		if (!delay && delay !== 0) return;

		const id = setInterval(() => savedCallback.current(), delay);

		return () => clearInterval(id);
	}, [delay, started]);

	return {
		start: () => !started && setStarted(true),
		stop: () => started && setStarted(false)
	};
}
