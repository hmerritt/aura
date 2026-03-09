import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { env } from "lib/global/env";
import { auraSettingsHost } from "lib/host/client";
import { type RendererMode, type SettingsDocument } from "lib/host/types";

import { Shader } from "view/components/experimental/Shader";

export const Route = createFileRoute("/")({
	component: IndexRoute
});

const IMAGE_MODE_PLACEHOLDER_SRC = `data:image/svg+xml;utf8,${encodeURIComponent(`
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 960 640" fill="none">
	<defs>
		<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
			<stop offset="0%" stop-color="#d7e6ff" />
			<stop offset="52%" stop-color="#edf4ff" />
			<stop offset="100%" stop-color="#f8f8f4" />
		</linearGradient>
		<linearGradient id="ridge" x1="0" y1="0" x2="1" y2="1">
			<stop offset="0%" stop-color="#415d79" />
			<stop offset="100%" stop-color="#6f8aa5" />
		</linearGradient>
		<linearGradient id="field" x1="0" y1="0" x2="1" y2="1">
			<stop offset="0%" stop-color="#8fa4bc" />
			<stop offset="100%" stop-color="#d7dfeb" />
		</linearGradient>
	</defs>
	<rect width="960" height="640" fill="url(#sky)" />
	<circle cx="748" cy="132" r="58" fill="#fffef7" opacity=".78" />
	<path d="M0 404c68-54 138-83 211-87 89-5 150 33 219 32 88-1 150-67 240-66 107 1 183 59 290 140v214H0V404Z" fill="url(#ridge)" />
	<path d="M0 476c82-48 170-76 260-70 109 8 162 65 267 65 91 0 170-49 252-46 74 3 135 31 181 66v146H0V476Z" fill="url(#field)" />
	<path d="M224 206c52 12 98 29 138 51" stroke="#ffffff" stroke-width="12" stroke-linecap="round" opacity=".55" />
</svg>
`)}`;

const SHADER_MODE_PREVIEW = `void mainImage(out vec4 fragColor, vec2 fragCoord) {
	vec2 uv = fragCoord / iResolution.xy;
	vec2 p = uv * 2.0 - 1.0;
	float glow = 0.25 / (0.2 + length(p - vec2(0.3 * sin(iTime * 0.7), 0.2 * cos(iTime * 0.5))));
	float wave = 0.5 + 0.5 * sin((p.x * 6.0 - p.y * 4.0) + iTime * 1.6);
	vec3 base = mix(vec3(0.05, 0.07, 0.12), vec3(0.08, 0.24, 0.52), uv.y + wave * 0.18);
	vec3 accent = vec3(0.22, 0.68, 1.0) * glow;
	fragColor = vec4(base + accent, 1.0);
}`;

export function IndexRoute() {
	const [document, setDocument] = useState<SettingsDocument | null>(null);
	const [hasLoadError, setHasLoadError] = useState(false);

	useEffect(() => {
		let isMounted = true;

		void auraSettingsHost
			.request("load_settings", {})
			.then((result) => {
				if (!isMounted) return;
				setDocument(result.document);
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

	const selectedRenderer = document?.renderer ?? null;
	const canSelectMode = document !== null;
	const setRenderer = (renderer: RendererMode) => {
		setDocument((current) => {
			if (!current) return current;
			return {
				...current,
				renderer
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
								preview={
									<img
										alt=""
										aria-hidden="true"
										data-testid="image-mode-preview"
										src={IMAGE_MODE_PLACEHOLDER_SRC}
										{...stylex.props(styles.media)}
									/>
								}
								disabled={!canSelectMode}
							/>

							<ModeOption
								description="Live shader viewport"
								isSelected={selectedRenderer === "shader"}
								label="Shader"
								mode="shader"
								onSelect={setRenderer}
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
	preview: React.ReactNode;
};

function ModeOption({
	description,
	disabled,
	isSelected,
	label,
	mode,
	onSelect,
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
				<span {...stylex.props(styles.previewFrame)}>{preview}</span>
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
		height: "190px",
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
