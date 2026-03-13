import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { env } from "lib/global/env";
import { auraSettingsHost } from "lib/host/client";
import { type RendererMode, type SettingsLoadResult } from "lib/host/types";

import { Shader } from "view/components/experimental/Shader";

export const Route = createFileRoute("/")({
	component: IndexRoute
});

const SHADER_MODE_PREVIEW = `void mainImage(out vec4 fragColor, vec2 fragCoord) {
    float mr = min(iResolution.x, iResolution.y);
    vec2 uv = (fragCoord * 2.0 - iResolution.xy) / mr;

    float d = -iTime * 0.8;
    float a = 0.0;
    for (float i = 0.0; i < 8.0; ++i) {
        a += cos(i - d - a * uv.x);
        d += sin(uv.y * i + a);
    }
    d += iTime * 0.5;

vec3 colorA = vec3(0.0, 0.4, 1); // Origin blue
vec3 colorB = vec3(1.0, 1.0, 1.0); // White
float t = cos(a) * 0.5 + 0.5;
vec3 col = mix(colorA, colorB, t);

    //col = cos(col * cos(vec3(d, a, 2.5)) * 0.8 + 0.5);
    fragColor = vec4(col, 1);
}`;

type LockedImageSelection = {
	id: string;
	src: string;
};

function toLockedImageSelection(
	id: string | null | undefined,
	src: string | null | undefined
): LockedImageSelection | null {
	if (!id || !src) return null;
	return { id, src };
}

export function IndexRoute() {
	const [settings, setSettings] = useState<SettingsLoadResult | null>(null);
	const [hasLoadError, setHasLoadError] = useState(false);
	const [lockedImageSelection, setLockedImageSelection] =
		useState<LockedImageSelection | null>(null);

	useEffect(() => {
		let isMounted = true;

		void auraSettingsHost
			.request("load_settings", {})
			.then((result) => {
				if (!isMounted) return;
				setSettings(result);
				setLockedImageSelection(
					result.document.renderer === "image"
						? toLockedImageSelection(
							result.imagePreview.currentId,
							result.imagePreview.currentSrc
						)
						: null
				);
				setHasLoadError(false);
			})
			.catch(() => {
				if (!isMounted) return;
				setHasLoadError(true);
			});

		return () => {
			isMounted = false;
		};
	}, []);

	const selectedRenderer = settings?.document.renderer ?? null;
	const canSelectMode = settings !== null;
	const previewFrame = settings?.previewFrame ?? { width: 16, height: 9 };
	const currentImageSelection = toLockedImageSelection(
		settings?.imagePreview.currentId,
		settings?.imagePreview.currentSrc
	);
	const nextImageSelection = toLockedImageSelection(
		settings?.imagePreview.nextId,
		settings?.imagePreview.nextSrc
	);
	const derivedImageSelection =
		selectedRenderer === "image"
			? (currentImageSelection ?? nextImageSelection)
			: (nextImageSelection ?? currentImageSelection);
	const imageModePreviewSrc =
		(lockedImageSelection ?? derivedImageSelection)?.src ?? null;
	const setRenderer = (renderer: RendererMode) => {
		if (renderer === "image" && settings && lockedImageSelection === null) {
			const intendedSelection =
				settings.document.renderer === "image"
					? (toLockedImageSelection(
						settings.imagePreview.currentId,
						settings.imagePreview.currentSrc
					) ??
						toLockedImageSelection(
							settings.imagePreview.nextId,
							settings.imagePreview.nextSrc
						))
					: (toLockedImageSelection(
						settings.imagePreview.nextId,
						settings.imagePreview.nextSrc
					) ??
						toLockedImageSelection(
							settings.imagePreview.currentId,
							settings.imagePreview.currentSrc
						));
			if (intendedSelection) {
				setLockedImageSelection(intendedSelection);
			}
		}

		setSettings((current) => {
			if (!current) return current;
			return {
				...current,
				document: {
					...current.document,
					renderer
				}
			};
		});
	};

	return (
		<div {...stylex.props(styles.page)}>
			<header data-testid="settings-header" {...stylex.props(styles.header)}>
				<div {...stylex.props(styles.brand)}>
					<img
						alt="aura logo"
						data-testid="settings-header-logo"
						src="/logo.png"
						draggable={false}
						{...stylex.props(styles.logo)}
					/>
					<h1 {...stylex.props(styles.title)}>aura</h1>
				</div>
				<p
					{...stylex.props(styles.version)}
				>{`Version ${env.appVersion ?? "unknown"}`}</p>
			</header>

			<main {...stylex.props(styles.content)}>
				<section
					aria-label="Mode selector"
					data-testid="mode-selector-section"
					{...stylex.props(styles.section)}
				>
					{hasLoadError && (
						<div {...stylex.props(styles.sectionHeader)}>
							<p {...stylex.props(styles.sectionMeta)}>
								Unable to load current settings.
							</p>
						</div>
					)}

					<fieldset {...stylex.props(styles.fieldset)}>
						<legend {...stylex.props(styles.legend)}>
							Choose how aura renders your desktop
						</legend>

						<div
							data-testid="mode-selector-grid"
							role="radiogroup"
							aria-label="Renderer mode"
							aria-busy={!canSelectMode}
							{...stylex.props(styles.optionGrid)}
						>
							<ModeOption
								isSelected={selectedRenderer === "image"}
								label="Image"
								mode="image"
								onSelect={setRenderer}
								previewFrame={previewFrame}
								preview={
									imageModePreviewSrc ? (
										<img
											alt=""
											draggable={false}
											aria-hidden="true"
											data-testid="image-mode-preview"
											src={imageModePreviewSrc}
											{...stylex.props(styles.media)}
										/>
									) : null
								}
								disabled={!canSelectMode}
							/>

							<ModeOption
								isSelected={selectedRenderer === "shader"}
								label="Shader"
								mode="shader"
								onSelect={setRenderer}
								previewFrame={previewFrame}
								preview={
									<Shader
										aria-hidden="true"
										data-testid="shader-mode-preview"
										input={{ inline: SHADER_MODE_PREVIEW }}
										sx={styles.media}
									/>
								}
								disabled={!canSelectMode}
							/>
						</div>
					</fieldset>
				</section>
			</main>
		</div>
	);
}

type ModeOptionProps = {
	description?: string;
	disabled: boolean;
	isSelected: boolean;
	label: string;
	mode: RendererMode;
	onSelect: (renderer: RendererMode) => void;
	previewFrame: {
		width: number;
		height: number;
	};
	preview: React.ReactNode;
};

function ModeOption({
	description,
	disabled,
	isSelected,
	label,
	mode,
	onSelect,
	previewFrame,
	preview
}: ModeOptionProps) {
	return (
		<label
			data-testid={`${mode}-mode-card`}
			{...stylex.props(styles.optionLabel, disabled && styles.optionLabelDisabled)}
		>
			<input
				aria-label={label}
				checked={isSelected}
				disabled={disabled}
				name="renderer-mode"
				onChange={() => onSelect(mode)}
				type="radio"
				value={mode}
				{...stylex.props(styles.radioInput)}
			/>

			<span {...stylex.props(styles.optionCard)}>
				<span
					data-testid={`${mode}-mode-preview-frame`}
					style={{
						aspectRatio: `${previewFrame.width} / ${previewFrame.height}`
					}}
					{...stylex.props(
						styles.previewFrame,
						isSelected && styles.optionCardSelected
					)}
				>
					{preview}
				</span>
			</span>
		</label>
	);
}

const styles = stylex.create({
	page: {
		width: "100%",
		minHeight: "100vh",
		backgroundColor: "#FFFFFF"
	},
	header: {
		width: "100%",
		height: "55px",
		paddingRight: "20px",
		paddingLeft: "20px",
		display: "flex",
		alignItems: "center",
		justifyContent: "space-between",
		backgroundColor: "#F2F2F2",
		borderBottomWidth: "1px",
		borderBottomStyle: "solid",
		borderBottomColor: "#DADADA"
	},
	brand: {
		display: "flex",
		alignItems: "center",
		gap: "15px"
	},
	logo: {
		width: "26px",
		height: "26px",
		objectFit: "contain",
		flexShrink: 0
	},
	title: {
		fontSize: "1.5rem",
		lineHeight: 1,
		fontWeight: 500,
		color: "#111111",
		textTransform: "lowercase"
	},
	version: {
		fontSize: "1.2rem",
		lineHeight: 1,
		fontWeight: 330,
		color: "#666666"
	},
	content: {
		paddingTop: "24px",
		paddingRight: "24px",
		paddingBottom: "24px",
		paddingLeft: "24px"
	},
	section: {
		display: "flex",
		flexDirection: "column",
		gap: "14px"
	},
	sectionHeader: {
		display: "flex",
		flexDirection: "column",
		gap: "6px"
	},
	sectionMeta: {
		fontSize: "1.2rem",
		lineHeight: 1.4,
		fontWeight: 500,
		color: "#8A4E2C"
	},
	fieldset: {
		borderWidth: 0,
		margin: 0,
		padding: 0
	},
	legend: {
		position: "absolute",
		width: "1px",
		height: "1px",
		padding: 0,
		margin: "-1px",
		overflow: "hidden",
		clip: "rect(0, 0, 0, 0)",
		borderWidth: 0
	},
	optionGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
		gap: "16px"
	},
	optionLabel: {
		display: "block",
		cursor: "pointer"
	},
	optionLabelDisabled: {
		cursor: "default"
	},
	radioInput: {
		position: "absolute",
		opacity: 0,
		width: "1px",
		height: "1px",
		pointerEvents: "none"
	},
	optionCard: {
		display: "flex",
		flexDirection: "column",
		gap: "14px"
	},
	optionCardSelected: {
		borderColor: "#00B7EC"
	},
	previewFrame: {
		display: "block",
		width: "100%",
		overflow: "hidden",
		backgroundColor: "#E5EAF2",
		borderWidth: "2px",
		borderStyle: "solid",
		borderColor: "rgba(0, 0, 0, 0.12)",
		borderRadius: "15px",
		boxShadow: {
			default: "rgba(100, 100, 111, 0.2) 0px 7px 29px 0px",
			":hover":
				"rgba(50, 50, 93, 0.25) 0px 30px 60px -12px, rgba(0, 0, 0, 0.3) 0px 18px 36px -18px"
		},
		transition:
			"transform 400ms ease, border-color 400ms ease, background-color 400ms ease, box-shadow 400ms ease",
		willChange: "transform, border-color, background-color, box-shadow, height",
		transform: { ":hover": "translateY(-1px)" }
	},
	media: {
		display: "block",
		width: "100%",
		height: "100%",
		objectFit: "cover"
	}
});
