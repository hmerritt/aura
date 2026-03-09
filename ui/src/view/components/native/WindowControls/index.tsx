import * as stylex from "@stylexjs/stylex";
import { FC } from "react";

import { Ripple } from "view/components/experimental/Ripple";

import { WindowIcon } from "./icons";

export type WindowControlsProps = {
	type?: "default" | "mac";
	onClose?: () => void;
	onMinimize?: () => void;
	onMaximize?: () => void;
};

/**
 * Controls to handle window minimisation, maximisation and closing.
 *
 * Used for frameless windows.
 */
export const WindowControls: FC<WindowControlsProps> = ({
	onClose,
	onMinimize,
	onMaximize
}) => {
	return (
		// @TODO Icons - separate icons for mac and windows?
		<div data-tauri-drag-region {...stylex.props(styles.drag, styles.controls)}>
			{onClose && (
				<div {...stylex.props(styles.controlMacContainer)} onClick={onClose}>
					<Ripple sx={[styles.controlMac, styles.controlMacClose]}>
						<WindowIcon name="MacClose" sx={styles.controlMacSvg} />
					</Ripple>
				</div>
			)}
			{onMinimize && (
				<div {...stylex.props(styles.controlMacContainer)} onClick={onMinimize}>
					<Ripple sx={[styles.controlMac, styles.controlMacMinimize]}>
						<WindowIcon name="MacMinimize" sx={styles.controlMacSvg} />
					</Ripple>
				</div>
			)}
			{onMaximize && (
				<div {...stylex.props(styles.controlMacContainer)} onClick={onMaximize}>
					<Ripple sx={[styles.controlMac, styles.controlMacMaximize]}>
						<WindowIcon name="MacMaximize" sx={styles.controlMacSvg} />
					</Ripple>
				</div>
			)}
		</div>
	);
};

/**
 * An area that can be used to drag the window around.
 */
export const WindowDragArea = () => {
	return <div data-tauri-drag-region {...stylex.props(styles.drag, styles.dragArea)} />;
};

const styles = stylex.create({
	drag: {
		"--runtime-draggable": "drag"
	},
	dragArea: {
		position: "fixed",
		top: 0,
		left: 0,
		right: 0,
		height: "2.5rem",
		zIndex: 99999
	},
	controls: {
		alignItems: "center",
		display: "flex",
		flexDirection: "row",
		gap: "1rem",
		justifyContent: "center",
		position: "relative"
	},
	controlMacContainer: {
		position: "relative",
		cursor: "pointer",
		"::before": {
			position: "absolute",
			content: "",
			top: -4,
			bottom: -4,
			left: -4,
			right: -4,
			borderRadius: "100%"
		},
		"--control-opacity": 0,
		// eslint-disable-next-line @stylexjs/valid-styles
		":hover": {
			"--control-opacity": 1
		}
	},
	controlMac: {
		alignItems: "center",
		borderRadius: "100%",
		display: "flex",
		height: "1.4rem",
		justifyContent: "center",
		width: "1.4rem"
	},
	controlMacClose: {
		backgroundColor: "#fc5753",
		borderColor: "#df4744",
		borderStyle: "solid",
		borderWidth: "0.1rem"
	},
	controlMacMaximize: {
		backgroundColor: "#33c748",
		borderColor: "#27aa35",
		// This button is sometimes gray ??!!
		// backgroundColor: "#ded8dc",
		// border: "0.1rem solid #cac4c8"
		borderStyle: "solid",
		borderWidth: "0.1rem"
	},
	controlMacMinimize: {
		backgroundColor: "#fdbc40",
		borderColor: "#de9f34",
		borderStyle: "solid",
		borderWidth: "0.1rem"
	},
	controlMacSvg: {
		height: "1.1rem",
		opacity: "var(--control-opacity)",
		transition: "opacity 150ms ease-in-out",
		width: "1.1rem"
	}
});
