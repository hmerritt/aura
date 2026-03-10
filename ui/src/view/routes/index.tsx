import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { env } from "lib/global/env";
import { auraSettingsHost } from "lib/host/client";
import {
	type RendererMode,
	type SettingsLoadResult
} from "lib/host/types";

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

export function IndexRoute() {
	const [settings, setSettings] = useState<SettingsLoadResult | null>(null);
	const [hasLoadError, setHasLoadError] = useState(false);

	useEffect(() => {
		let isMounted = true;

		void auraSettingsHost
			.request("load_settings", {})
			.then((result) => {
				if (!isMounted) return;
				setSettings(result);
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
	const imageModePreviewSrc =
		selectedRenderer === "image"
			? settings?.imagePreview.currentSrc ?? settings?.imagePreview.nextSrc ?? null
			: settings?.imagePreview.nextSrc ?? settings?.imagePreview.currentSrc ?? null;
	const setRenderer = (renderer: RendererMode) => {
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
				<p {...stylex.props(styles.version)}>{`Version ${env.appVersion ?? "unknown"}`}</p>
			</header>

			<main {...stylex.props(styles.content)}>
				<section
					aria-label="Mode selector"
					data-testid="mode-selector-section"
					{...stylex.props(styles.section)}
				>
					<div {...stylex.props(styles.sectionHeader)}>
						<h2 {...stylex.props(styles.sectionTitle)}>Mode</h2>
						{hasLoadError && (
							<p {...stylex.props(styles.sectionMeta)}>Unable to load current settings.</p>
						)}
					</div>

					<fieldset {...stylex.props(styles.fieldset)}>
						<legend {...stylex.props(styles.legend)}>Choose how aura renders your desktop</legend>

						<div
							data-testid="mode-selector-grid"
							role="radiogroup"
							aria-label="Renderer mode"
							aria-busy={!canSelectMode}
							{...stylex.props(styles.optionGrid)}
						>
							<ModeOption
								description="Still wallpaper preview"
								isSelected={selectedRenderer === "image"}
								label="Image"
								mode="image"
								onSelect={setRenderer}
								previewFrame={previewFrame}
								preview={
									imageModePreviewSrc ? (
										<img
											alt=""
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
								description="Live shader viewport"
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
	description: string;
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
			{...stylex.props(
				styles.optionLabel,
				disabled && styles.optionLabelDisabled
			)}
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

			<span
				{...stylex.props(
					styles.optionCard,
					isSelected && styles.optionCardSelected
				)}
			>
				<span
					data-testid={`${mode}-mode-preview-frame`}
					style={{ aspectRatio: `${previewFrame.width} / ${previewFrame.height}` }}
					{...stylex.props(styles.previewFrame)}
				>
					{preview}
				</span>
				<span {...stylex.props(styles.optionMeta)}>
					<span
						{...stylex.props(
							styles.optionTitle,
							isSelected && styles.optionTitleSelected
						)}
					>
						{label}
					</span>
					<span {...stylex.props(styles.optionDescription)}>{description}</span>
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
		paddingTop: "15px",
		paddingRight: "24px",
		paddingBottom: "15px",
		paddingLeft: "24px",
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
		fontSize: "1.3rem",
		lineHeight: 1,
		fontWeight: 500,
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
	sectionTitle: {
		fontSize: "1.6rem",
		lineHeight: 1,
		fontWeight: 600,
		color: "#111111"
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
		gap: "14px",
		paddingTop: "14px",
		paddingRight: "14px",
		paddingBottom: "16px",
		paddingLeft: "14px",
		borderWidth: "1px",
		borderStyle: "solid",
		borderColor: "#D7DCE4",
		borderRadius: "20px",
		backgroundColor: "#F7F8FA",
		transition:
			"transform 160ms ease, border-color 160ms ease, background-color 160ms ease, box-shadow 160ms ease",
		boxShadow: "0 10px 24px rgba(20, 33, 61, 0.04)",
		":hover": {
			transform: "translateY(-1px)",
			backgroundColor: "#FBFCFD",
			boxShadow: "0 14px 28px rgba(20, 33, 61, 0.07)"
		}
	},
	optionCardSelected: {
		borderColor: "#2D6BFF",
		backgroundColor: "#F4F8FF",
		boxShadow: "0 0 0 2px rgba(45, 107, 255, 0.12), 0 16px 30px rgba(29, 64, 128, 0.10)"
	},
	previewFrame: {
		display: "block",
		width: "100%",
		overflow: "hidden",
		borderRadius: "14px",
		backgroundColor: "#E5EAF2",
		borderWidth: "1px",
		borderStyle: "solid",
		borderColor: "rgba(88, 104, 134, 0.14)"
	},
	media: {
		display: "block",
		width: "100%",
		height: "100%",
		objectFit: "cover"
	},
	optionMeta: {
		display: "flex",
		flexDirection: "column",
		gap: "6px"
	},
	optionTitle: {
		fontSize: "1.7rem",
		lineHeight: 1,
		fontWeight: 600,
		color: "#20242D"
	},
	optionTitleSelected: {
		color: "#1742B0"
	},
	optionDescription: {
		fontSize: "1.2rem",
		lineHeight: 1.3,
		fontWeight: 500,
		color: "#6F7786"
	}
});
