import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";

import { shadowFn } from "lib/styles/shadows.stylex";

import { FrostedGlass } from "view/components/experimental/FrostedGlass";
import { Shader } from "view/components/experimental/Shader";

export const Route = createFileRoute("/")({
	component: IndexRoute
});

export function IndexRoute() {
	return (
		<div {...stylex.props(styles.container)}>
			<h1 {...stylex.props(styles.header, shadowFn.textBlock("#070707"))}>
				aura
			</h1>
			<FrostedGlass>
				<h4 {...stylex.props(styles.subtitle)}>
					aura 🔋
				</h4>
			</FrostedGlass>
			<Shader
				sx={styles.shader}
				input={{
					inline: `void mainImage(out vec4 fragColor, vec2 fragCoord) {
						float mr = min(iResolution.x, iResolution.y);
						vec2 uv = (fragCoord * 2.0 - iResolution.xy) / mr;

						float d = -iTime * 0.8;
						float a = 0.0;
						for (float i = 0.0; i < 8.0; ++i) {
							a += cos(i - d - a * uv.x);
							d += sin(uv.y * i + a);
						}
						d += iTime * 0.5;

						vec3 colorA = vec3(0.0, 0.4, 1); // Blue
						vec3 colorB = vec3(.03, .03, .03); // Black
						float t = cos(a) * 0.5 + 0.5;
						vec3 col = mix(colorA, colorB, t);

						fragColor = vec4(col, 1);
					}`
				}}
			/>
		</div>
	);
}

const styles = stylex.create({
	container: {
		display: "flex",
		position: "fixed",
		top: 0,
		left: 0,
		width: "100%",
		height: "100%",
		alignItems: "center",
		justifyContent: "center",
		flexDirection: "column",
		backgroundColor: "#070707"
	},
	header: {
		color: "#fff",
		fontSize: "10rem",
		fontStyle: "italic",
		fontWeight: "bold",
		textTransform: "lowercase"
	},
	subtitle: {
		color: "#fff",
		fontSize: "1.5rem",
		fontStyle: "italic",
		opacity: 0.8,
		padding: "1rem"
	},
	shader: {
		position: "absolute",
		top: 0,
		left: 0,
		width: "100%",
		height: "100%",
		zIndex: -1,
		backgroundColor: "#070707"
	}
});
