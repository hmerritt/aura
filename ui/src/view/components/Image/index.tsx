import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { type SxProp } from "lib/type-assertions";

export type ImageProps = React.JSX.IntrinsicElements["img"] &
	SxProp & {
		/** Maintains image aspect ratio - even when render width/height are fluid */
		aspectRatioMaintain?: boolean;
		/** Hides image while loading */
		hideWhileLoading?: boolean;
		/** Image source that is shown if `src` image fails to load (also shown while loading if `loadingSrc` is unset) */
		fallbackSrc?: string;
		/** Image source that is shown while `src` image is loading */
		loadingSrc?: string;
	};

export const Image = ({
	sx,
	aspectRatioMaintain,
	fallbackSrc,
	height,
	hideWhileLoading,
	loadingSrc,
	src,
	width,
	onError,
	onLoad,
	...props
}: ImageProps) => {
	if (aspectRatioMaintain && (width == null || height == null)) {
		debug(
			"Image",
			"warn",
			"aspectRatioMaintain requires both width and height props"
		);
	}

	const usesFallback = !!fallbackSrc || !!loadingSrc;

	const [{ isLoading, hasError }, setState] = useState({
		isLoading: true,
		hasError: false
	});

	return (
		<>
			<img
				src={src}
				{...stylex.props(
					aspectRatioMaintain && styles.arm,
					hideWhileLoading && isLoading && styles.hidden, // Hide while loading
					!usesFallback && hasError && styles.hidden, // Hide when errored, and no fallback set
					usesFallback && (isLoading || hasError) && styles.none, // Hide when fallback is active
					sx
				)}
				draggable={false} // <- All websites should do this my my
				width={width}
				height={height}
				alt=""
				{...((onLoad || usesFallback || hideWhileLoading) && {
					onLoad: (e) => {
						if (onLoad) onLoad(e);
						setState((prev) => ({ ...prev, isLoading: false }));
					}
				})}
				{...((onError || usesFallback) && {
					onError: (e) => {
						if (onError) onError(e);
						setState((prev) => ({
							...prev,
							isLoading: false,
							hasError: true
						}));
					}
				})}
				{...props}
			/>

			{usesFallback && (
				<img
					src={isLoading && loadingSrc ? loadingSrc : fallbackSrc}
					{...stylex.props(
						aspectRatioMaintain && styles.arm,
						!isLoading && !hasError && styles.none,
						sx
					)}
					draggable={false}
					width={width}
					height={height}
					alt=""
					{...props}
				/>
			)}
		</>
	);
};

const styles = stylex.create({
	// aspectRatioMaintain
	arm: {
		height: "auto",
		maxWidth: "100%"
	},
	hidden: {
		opacity: 0
	},
	none: {
		display: "none"
	}
});
